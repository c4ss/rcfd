//! Linear multi-point constraints by the transformation `u = T q + g`, rigid sets with a reference point, elastic supports
//! ([`crate::model::Constraint::Linear`], [`crate::model::Constraint::RigidSet`], [`crate::model::ElasticSupport`]).
//! Declared by ADR 0291; filled by ADR 0296.
//!
//! The unknowns are `x = (u, r)`: the mesh unknowns of the model's [`crate::model::DofMap`] and six auxiliary unknowns per
//! rigid set (the translation `t` and the small rotation `w` of its reference point), numbered after the mesh's in the order
//! of the constraints. Every constraint is a linear row `c . x = b`: a prescribed displacement `u_i = g_i`, a linear
//! constraint `sum a_k u_k = b`, and for each node `a` of a rigid set and each component `u_a - t - w x (x_a - x_R) = 0`.
//! The rows are eliminated in a fixed order with designated dependents (the unknown of a prescribed row, the first term of
//! a linear constraint, the node component of a rigid-set row) by Gauss-Jordan substitution, so that every dependent is an
//! expression `x_d = sum e_dk x_k + h_d` of independent unknowns only; then `x = T q + g` with `q` the independent unknowns
//! (ascending), and the potential energy restricted to admissible `x` gives `K_r q = f_r`, `K_r = T^T K_ext T`, `f_r = T^T
//! (f_ext - K_ext g)` (master-slave elimination as a congruence transformation; ADR 0296, derivation there). A row implied by
//! earlier ones is dropped and reported (redundant) or refused (contradictory); a row whose dependent appears in the
//! expression of another dependent of the row closes a chain on itself and is refused. The constraint force `rho = K_ext x -
//! f_ext` satisfies `T^T rho = K_r q - f_r`: it does no work on admissible motions.
//!
//! Elastic supports are stiffness, added to `K` before the transformation: a nodal spring on the diagonal, a foundation as
//! the consistent `int k_f N_a N_b n n^T dA` over the faces of a surface set (the face functions and rule of
//! [`crate::loads`]). A model with prescribed displacements only never comes here: [`crate::solve::solve_static_report`]
//! dispatches on [`uses_transformation`].

use crate::assemble::stiffness_with;
use crate::constraint::{prescribed_equations, Prescribed};
use crate::error::{FemError, Result, SetKind};
use crate::loads::{face_rule, face_shape_functions, nodal_load_vector, NodalForceAt};
use crate::mass::consistent_with;
use crate::modal::{ModalOptions, ModalResult};
use crate::model::{Constraint, ElasticSupport, SolidModel, SolidResult, SolverStats};
use crate::solve::{SolveReport, StaticOptions, StaticSolver, PIVOT_TOLERANCE};
use rcfd_core::vec3::{cross, norm, Vec3};
use rcfd_linalg::eigen::{lowest_eigenpairs, Shift, SubspaceOptions};
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt, SkylineOptions};
use rcfd_linalg::{cg, CsrMatrix, Jacobi, SolveControl};
use std::collections::{BTreeMap, BTreeSet};

/// The work package that adds linear constraints, rigid sets and elastic supports; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Relative threshold of the dependence decision (ADR 0296): a substituted coefficient `|c| <= tau s` is a rounding residue,
/// and a row implied by earlier ones is redundant if `|b'| <= tau s_b`, contradictory otherwise.
pub const DEPENDENCE_TOLERANCE: f64 = 1e-10;

/// Unknowns of one reference point: the translation (0, 1, 2) and the small rotation (3, 4, 5).
pub const REFERENCE_UNKNOWNS: usize = 6;

/// Whether a model needs the transformation: it has a linear constraint, a rigid set, a cyclic constraint (ADR 0386) or an
/// elastic support.
pub fn uses_transformation(model: &SolidModel) -> bool {
    !model.supports().is_empty()
        || model.constraints().iter().any(|c| {
            matches!(
                c,
                Constraint::Linear { .. } | Constraint::RigidSet { .. } | Constraint::Cyclic { .. }
            )
        })
}

/// Where a constraint row comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RowOrigin {
    /// A prescribed displacement (of a `Displacement` constraint or of the per-node list).
    Prescribed {
        /// Node.
        node: usize,
        /// Component.
        component: usize,
    },
    /// A linear constraint, by its index in the model's constraints.
    Linear {
        /// Index in [`SolidModel::constraints`].
        constraint: usize,
    },
    /// One row of a rigid set: one component of one node of its set.
    RigidSet {
        /// Index in [`SolidModel::constraints`].
        constraint: usize,
        /// Node of the set.
        node: usize,
        /// Component.
        component: usize,
    },
    /// One row of a cyclic constraint (ADR 0386): one component of one node of its high boundary.
    Cyclic {
        /// Index in [`SolidModel::constraints`].
        constraint: usize,
        /// Node of the high boundary.
        node: usize,
        /// Component.
        component: usize,
    },
}

impl RowOrigin {
    /// The index of the model's constraint, if the row comes from one (not for a prescribed displacement).
    pub fn constraint(&self) -> Option<usize> {
        match *self {
            RowOrigin::Prescribed { .. } => None,
            RowOrigin::Linear { constraint }
            | RowOrigin::RigidSet { constraint, .. }
            | RowOrigin::Cyclic { constraint, .. } => Some(constraint),
        }
    }

    /// A description naming the constraint: "constraint 4 (Linear)", "constraint 2 (RigidSet 'tip', node 17 component 1)" or
    /// "the prescribed displacement of node 3 component 0".
    pub fn description(&self, model: &SolidModel) -> String {
        match *self {
            RowOrigin::Prescribed { node, component } => {
                format!("the prescribed displacement of node {node} component {component}")
            }
            RowOrigin::Linear { constraint } => format!("constraint {constraint} (Linear)"),
            RowOrigin::RigidSet {
                constraint,
                node,
                component,
            } => {
                let set = match model.constraints().get(constraint) {
                    Some(Constraint::RigidSet { set, .. }) => set.as_str(),
                    _ => "?",
                };
                format!("constraint {constraint} (RigidSet '{set}', node {node} component {component})")
            }
            RowOrigin::Cyclic {
                constraint,
                node,
                component,
            } => {
                let (low, high) = match model.constraints().get(constraint) {
                    Some(Constraint::Cyclic { low, high, .. }) => (low.as_str(), high.as_str()),
                    _ => ("?", "?"),
                };
                format!("constraint {constraint} (Cyclic '{low}'/'{high}', high node {node} component {component})")
            }
        }
    }
}

/// A row dropped as redundant: implied by earlier rows, with the residue of its value.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct DroppedRow {
    /// The row.
    pub origin: RowOrigin,
    /// `|b'|`, the value left after the substitution of the earlier rows.
    pub residual: f64,
    /// The value scale `s_b` it is compared with.
    pub scale: f64,
}

/// A force and a moment at the reference point of a rigid set.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ReferenceLoad {
    /// Index of the rigid set in [`SolidModel::constraints`].
    pub constraint: usize,
    /// Force, N.
    pub force: Vec3,
    /// Moment about the reference point, N m.
    pub moment: Vec3,
}

impl ReferenceLoad {
    /// A force and a moment at the reference point of rigid set `constraint`.
    pub fn new(constraint: usize, force: Vec3, moment: Vec3) -> ReferenceLoad {
        ReferenceLoad {
            constraint,
            force,
            moment,
        }
    }
}

/// The motion of the reference point of a rigid set.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ReferenceMotion {
    /// Index of the rigid set in [`SolidModel::constraints`].
    pub constraint: usize,
    /// Translation, m.
    pub translation: Vec3,
    /// Small rotation, rad.
    pub rotation: Vec3,
}

#[derive(Debug, Clone, Default)]
struct Expr {
    coef: BTreeMap<usize, f64>,
    constant: f64,
    sources: BTreeSet<RowOrigin>,
}

struct Row {
    origin: RowOrigin,
    terms: Vec<(usize, f64)>,
    value: f64,
    designated: usize,
}

/// The transformation `x = T q + g` of a model's constraints (ADR 0296).
#[derive(Debug, Clone, PartialEq)]
pub struct Transformation {
    t: CsrMatrix,
    g: Vec<f64>,
    independent: Vec<usize>,
    n_mesh: usize,
    references: Vec<(usize, usize)>,
    dropped: Vec<DroppedRow>,
    constrained: Vec<bool>,
}

impl Transformation {
    /// `T`, `N x n_q` (`N` = mesh unknowns + six per rigid set).
    pub fn t(&self) -> &CsrMatrix {
        &self.t
    }
    /// `g`, length `N`.
    pub fn g(&self) -> &[f64] {
        &self.g
    }
    /// The independent unknowns, ascending: `q_i = x_(independent()[i])`.
    pub fn independent(&self) -> &[usize] {
        &self.independent
    }
    /// The number of mesh unknowns `n` (the reference unknowns are `n..N`).
    pub fn n_mesh(&self) -> usize {
        self.n_mesh
    }
    /// The number of all unknowns `N`.
    pub fn n_unknowns(&self) -> usize {
        self.g.len()
    }
    /// The first of the six unknowns of the reference point of rigid set `constraint`, if it is one.
    pub fn reference_offset(&self, constraint: usize) -> Option<usize> {
        self.references.iter().find(|&&(c, _)| c == constraint).map(|&(_, o)| o)
    }
    /// The rigid sets as (constraint index, first reference unknown), in the order of the constraints.
    pub fn references(&self) -> &[(usize, usize)] {
        &self.references
    }
    /// The rows dropped as redundant, in the order processed.
    pub fn dropped(&self) -> &[DroppedRow] {
        &self.dropped
    }
    /// Whether unknown `i` takes part in a constraint row (as its unknown or through an expression).
    pub fn is_constrained(&self, i: usize) -> bool {
        self.constrained[i]
    }
    /// `x = T q + g`.
    pub fn expand(&self, q: &[f64]) -> Vec<f64> {
        assert_eq!(q.len(), self.independent.len(), "reduced solution length");
        let mut x = self.g.clone();
        for (p, xp) in x.iter_mut().enumerate() {
            let mut s = 0.0;
            for k in self.t.row_ptr[p]..self.t.row_ptr[p + 1] {
                s += self.t.val[k] * q[self.t.col[k]];
            }
            *xp += s;
        }
        x
    }
    /// `T^T v` for a vector over all `N` unknowns.
    pub fn transpose_times(&self, v: &[f64]) -> Vec<f64> {
        assert_eq!(v.len(), self.g.len(), "vector length");
        let mut out = vec![0.0; self.independent.len()];
        for (p, &vp) in v.iter().enumerate() {
            for k in self.t.row_ptr[p]..self.t.row_ptr[p + 1] {
                out[self.t.col[k]] += self.t.val[k] * vp;
            }
        }
        out
    }
    /// `K_r = T^T K T` and `f_r = T^T (f - K g)` for the mesh stiffness `k` (`n x n`; the reference rows and columns are
    /// zero) and a load vector `f` over all `N` unknowns.
    pub fn reduce(&self, k: &CsrMatrix, f: &[f64]) -> Result<(CsrMatrix, Vec<f64>)> {
        let n = self.n_mesh;
        let nn = self.g.len();
        let nq = self.independent.len();
        if k.n != n || k.ncols != n {
            return Err(FemError::FieldLength {
                key: "stiffness",
                expected: n,
                found: k.n,
            });
        }
        if f.len() != nn {
            return Err(FemError::FieldLength {
                key: "load vector",
                expected: nn,
                found: f.len(),
            });
        }
        // A = K T, row by row over the mesh rows (the reference rows of K are zero).
        let mut acc = vec![0.0f64; nq];
        let mut mark = vec![usize::MAX; nq];
        let mut a_ptr = vec![0usize];
        let (mut a_col, mut a_val) = (Vec::new(), Vec::new());
        for i in 0..n {
            let mut cols = Vec::new();
            for p in k.row_ptr[i]..k.row_ptr[i + 1] {
                let (j, kij) = (k.col[p], k.val[p]);
                for s in self.t.row_ptr[j]..self.t.row_ptr[j + 1] {
                    let alpha = self.t.col[s];
                    if mark[alpha] != i {
                        mark[alpha] = i;
                        acc[alpha] = 0.0;
                        cols.push(alpha);
                    }
                    acc[alpha] += kij * self.t.val[s];
                }
            }
            cols.sort_unstable();
            for alpha in cols {
                a_col.push(alpha);
                a_val.push(acc[alpha]);
            }
            a_ptr.push(a_col.len());
        }
        // The transpose of T (columns of T as rows), ascending rows.
        let mut tt: Vec<Vec<(usize, f64)>> = vec![Vec::new(); nq];
        for p in 0..nn {
            for s in self.t.row_ptr[p]..self.t.row_ptr[p + 1] {
                tt[self.t.col[s]].push((p, self.t.val[s]));
            }
        }
        // K_r row alpha = sum_p T(p, alpha) A(p, :).
        let mut mark = vec![usize::MAX; nq];
        let mut row_ptr = vec![0usize];
        let (mut col, mut val) = (Vec::new(), Vec::new());
        for (alpha, column) in tt.iter().enumerate() {
            let mut cols = Vec::new();
            for &(p, tp) in column {
                if p >= n {
                    continue;
                }
                for s in a_ptr[p]..a_ptr[p + 1] {
                    let beta = a_col[s];
                    if mark[beta] != alpha {
                        mark[beta] = alpha;
                        acc[beta] = 0.0;
                        cols.push(beta);
                    }
                    acc[beta] += tp * a_val[s];
                }
            }
            cols.sort_unstable();
            for beta in cols {
                col.push(beta);
                val.push(acc[beta]);
            }
            row_ptr.push(col.len());
        }
        let kr = CsrMatrix::rect(nq, nq, row_ptr, col, val);
        // f_r = T^T (f - K g).
        let mut kg = vec![0.0; n];
        k.matvec(&self.g[..n], &mut kg);
        let mut y = f.to_vec();
        for (yi, kgi) in y.iter_mut().zip(&kg) {
            *yi -= kgi;
        }
        Ok((kr, self.transpose_times(&y)))
    }
}

fn constraint_error(reason: String) -> FemError {
    FemError::InvalidValue {
        key: "constraint",
        reason,
    }
}

fn describe_list(model: &SolidModel, origins: &BTreeSet<RowOrigin>, except: RowOrigin) -> String {
    const MAX: usize = 12;
    let list: Vec<String> = origins
        .iter()
        .filter(|&&o| o != except)
        .take(MAX)
        .map(|o| o.description(model))
        .collect();
    let more = origins.iter().filter(|&&o| o != except).count().saturating_sub(MAX);
    let mut s = list.join(", ");
    if more > 0 {
        s.push_str(&format!(" and {more} more"));
    }
    if s.is_empty() {
        s.push_str("itself");
    }
    s
}

struct Elimination<'a> {
    model: &'a SolidModel,
    n_mesh: usize,
    // The sine part of a doubled harmonic problem (ADR 0386): unknowns `n_mesh .. 2 n_mesh` are named as mesh unknowns.
    sine: bool,
    deps: BTreeMap<usize, Expr>,
    users: BTreeMap<usize, BTreeSet<usize>>,
    dropped: Vec<DroppedRow>,
    constrained: Vec<bool>,
}

impl Elimination<'_> {
    fn name(&self, i: usize) -> String {
        if i < self.n_mesh {
            let (node, component) = self.model.dof_map().node_component(i).expect("mesh unknown");
            format!("node {node} component {component}")
        } else if self.sine && i < 2 * self.n_mesh {
            let (node, component) = self
                .model
                .dof_map()
                .node_component(i - self.n_mesh)
                .expect("mesh unknown");
            format!("node {node} component {component} (sine part)")
        } else {
            // The reference unknowns follow the sine part in a doubled harmonic problem (ADR 0395).
            let base = if self.sine { 2 * self.n_mesh } else { self.n_mesh };
            format!("reference unknown {}", i - base)
        }
    }

    fn process(&mut self, row: Row) -> Result<()> {
        let tau = DEPENDENCE_TOLERANCE;
        let d = row.designated;
        let d_dependent = self.deps.contains_key(&d);
        let mut c: BTreeMap<usize, f64> = BTreeMap::new();
        let mut scale: f64 = 0.0;
        let mut b = row.value;
        let mut sb = row.value.abs();
        let mut sources: BTreeSet<RowOrigin> = BTreeSet::new();
        sources.insert(row.origin);
        let mut chain: BTreeSet<RowOrigin> = BTreeSet::new();
        for &(j, a) in &row.terms {
            if a == 0.0 {
                continue;
            }
            self.constrained[j] = true;
            scale = scale.max(a.abs());
            match self.deps.get(&j) {
                Some(e) => {
                    if !d_dependent && e.coef.contains_key(&d) {
                        chain.extend(e.sources.iter().copied());
                    }
                    for (&kk, &ek) in &e.coef {
                        let v = a * ek;
                        scale = scale.max(v.abs());
                        *c.entry(kk).or_insert(0.0) += v;
                    }
                    b -= a * e.constant;
                    sb += (a * e.constant).abs();
                    sources.extend(e.sources.iter().copied());
                }
                None => *c.entry(j).or_insert(0.0) += a,
            }
        }
        if !chain.is_empty() {
            return Err(constraint_error(format!(
                "{} closes a chain on {} through {}: its dependent unknown depends on itself",
                row.origin.description(self.model),
                self.name(d),
                describe_list(self.model, &chain, row.origin)
            )));
        }
        c.retain(|_, v| v.abs() > tau * scale);
        let pivot = if !d_dependent && c.contains_key(&d) {
            d
        } else if c.is_empty() {
            if b.abs() <= tau * sb {
                self.dropped.push(DroppedRow {
                    origin: row.origin,
                    residual: b.abs(),
                    scale: sb,
                });
                return Ok(());
            }
            let defining = self.deps.get(&d).map(|e| e.sources.clone()).unwrap_or_default();
            let mut named = defining;
            named.extend(sources.iter().copied());
            return Err(constraint_error(format!(
                "{} contradicts {}: after their substitution the row reads 0 = {b:e} on {} (value scale {sb:e})",
                row.origin.description(self.model),
                describe_list(self.model, &named, row.origin),
                self.name(d)
            )));
        } else {
            // The designated dependent is taken: the largest remaining coefficient (lowest index on ties).
            let mut best = (0usize, 0.0f64);
            for (&kk, &v) in &c {
                if v.abs() > best.1 {
                    best = (kk, v.abs());
                }
            }
            best.0
        };
        let cp = c.remove(&pivot).expect("pivot coefficient");
        let mut expr = Expr {
            coef: BTreeMap::new(),
            constant: b / cp,
            sources,
        };
        for (&kk, &v) in &c {
            expr.coef.insert(kk, -v / cp);
        }
        self.constrained[pivot] = true;
        // Gauss-Jordan: substitute the new dependent into every expression that contains it.
        if let Some(users) = self.users.remove(&pivot) {
            for u in users {
                let e = self.deps.get_mut(&u).expect("user is a dependent");
                let ep = e.coef.remove(&pivot).expect("user contains the pivot");
                for (&kk, &v) in &expr.coef {
                    let add = ep * v;
                    let entry = e.coef.entry(kk).or_insert(0.0);
                    let old = *entry;
                    *entry += add;
                    if entry.abs() <= tau * old.abs().max(add.abs()) {
                        e.coef.remove(&kk);
                        if let Some(s) = self.users.get_mut(&kk) {
                            s.remove(&u);
                        }
                    } else {
                        self.users.entry(kk).or_default().insert(u);
                    }
                }
                e.constant += ep * expr.constant;
                e.sources.extend(expr.sources.iter().copied());
            }
        }
        for &kk in expr.coef.keys() {
            self.users.entry(kk).or_default().insert(pivot);
        }
        self.deps.insert(pivot, expr);
        Ok(())
    }
}

/// The transformation of a model's constraints and of the per-node prescribed values `prescribed` (ADR 0296; the `k = 0`
/// rows `u_H = R u_L` of a cyclic constraint, ADR 0386). Refuses a contradictory row and a row that closes a chain on
/// itself, naming the constraints; drops and lists a redundant row.
pub fn transformation(model: &SolidModel, prescribed: &[Prescribed]) -> Result<Transformation> {
    let fixed = prescribed_equations(model, prescribed)?;
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let mesh = model.mesh();
    let mut references = Vec::new();
    let mut nn = n;
    for (i, c) in model.constraints().iter().enumerate() {
        if let Constraint::RigidSet { .. } = c {
            references.push((i, nn));
            nn += REFERENCE_UNKNOWNS;
        }
    }
    let mut rows: Vec<Row> = Vec::new();
    for &(eq, v) in &fixed {
        let (node, component) = dofs.node_component(eq).expect("prescribed equation");
        rows.push(Row {
            origin: RowOrigin::Prescribed { node, component },
            terms: vec![(eq, 1.0)],
            value: v,
            designated: eq,
        });
    }
    for (i, c) in model.constraints().iter().enumerate() {
        match c {
            Constraint::Linear { terms, value } => {
                // Merge repeated unknowns in the order of their first appearance.
                let mut merged: Vec<(usize, f64)> = Vec::new();
                for t in terms {
                    let eq = dofs.equation(t.node, t.component).ok_or(FemError::MissingComponent {
                        role: "linear constraint",
                        node: t.node,
                        component: t.component,
                        available: dofs.components(t.node),
                    })?;
                    match merged.iter_mut().find(|m| m.0 == eq) {
                        Some(m) => m.1 += t.coefficient,
                        None => merged.push((eq, t.coefficient)),
                    }
                }
                let origin = RowOrigin::Linear { constraint: i };
                match merged.iter().find(|m| m.1 != 0.0) {
                    Some(&(designated, _)) => rows.push(Row {
                        origin,
                        terms: merged,
                        value: *value,
                        designated,
                    }),
                    None => {
                        return Err(constraint_error(format!(
                            "{}: its coefficients cancel to zero",
                            origin.description(model)
                        )))
                    }
                }
            }
            Constraint::RigidSet { set, reference } => {
                let nodes = mesh.node_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: SetKind::Node,
                    name: set.clone(),
                })?;
                let r0 = references.iter().find(|&&(c, _)| c == i).expect("reference").1;
                let (t0, w0) = (r0, r0 + 3);
                for &node in nodes {
                    let x = mesh.nodes()[node];
                    let dv = [x[0] - reference[0], x[1] - reference[1], x[2] - reference[2]];
                    for comp in 0..3 {
                        let eq = dofs.equation(node, comp).ok_or(FemError::MissingComponent {
                            role: "rigid set",
                            node,
                            component: comp,
                            available: dofs.components(node),
                        })?;
                        // u_c - t_c - (w x d)_c = 0.
                        let (j, k) = ((comp + 1) % 3, (comp + 2) % 3);
                        let mut terms = vec![(eq, 1.0), (t0 + comp, -1.0)];
                        if dv[k] != 0.0 {
                            terms.push((w0 + j, -dv[k]));
                        }
                        if dv[j] != 0.0 {
                            terms.push((w0 + k, dv[j]));
                        }
                        rows.push(Row {
                            origin: RowOrigin::RigidSet {
                                constraint: i,
                                node,
                                component: comp,
                            },
                            terms,
                            value: 0.0,
                            designated: eq,
                        });
                    }
                }
            }
            // The k = 0 rows u_H - R u_L = 0 of a cyclic constraint (ADR 0386).
            Constraint::Cyclic { .. } => {
                let bd = crate::cyclic::boundary(model, i)?;
                rows.extend(cyclic_rows(model, i, &bd, 1.0, 0.0, None)?);
            }
            // Displacement constraints are in `fixed`.
            _ => {}
        }
    }
    eliminate(model, n, nn, n, references, rows, false)
}

/// The rows of a cyclic constraint for the phase `(c, s) = (cos theta, sin theta)`: for each matched pair and component,
/// `a_H - R (c a_L - s b_L) = 0` and, with a sine part at offset `sine`, `b_H - R (s a_L + c b_L) = 0` (ADR 0386).
fn cyclic_rows(
    model: &SolidModel,
    constraint: usize,
    bd: &crate::cyclic::CyclicBoundary,
    c: f64,
    s: f64,
    sine: Option<usize>,
) -> Result<Vec<Row>> {
    let dofs = model.dof_map();
    let eq = |node: usize, comp: usize| {
        dofs.equation(node, comp).ok_or(FemError::MissingComponent {
            role: "cyclic constraint",
            node,
            component: comp,
            available: dofs.components(node),
        })
    };
    let r = bd.rotation();
    let mut rows = Vec::new();
    for &(low, high) in bd.pairs() {
        for comp in 0..3 {
            let origin = RowOrigin::Cyclic {
                constraint,
                node: high,
                component: comp,
            };
            let h = eq(high, comp)?;
            let mut terms = vec![(h, 1.0)];
            for (j, &rj) in r[comp].iter().enumerate() {
                let l = eq(low, j)?;
                if rj != 0.0 {
                    if c != 0.0 {
                        terms.push((l, -c * rj));
                    }
                    if let (Some(off), true) = (sine, s != 0.0) {
                        terms.push((off + l, s * rj));
                    }
                }
            }
            rows.push(Row {
                origin,
                terms,
                value: 0.0,
                designated: h,
            });
            if let Some(off) = sine {
                let mut terms = vec![(off + h, 1.0)];
                for (j, &rj) in r[comp].iter().enumerate() {
                    let l = eq(low, j)?;
                    if rj != 0.0 {
                        if s != 0.0 {
                            terms.push((l, -s * rj));
                        }
                        if c != 0.0 {
                            terms.push((off + l, -c * rj));
                        }
                    }
                }
                rows.push(Row {
                    origin,
                    terms,
                    value: 0.0,
                    designated: off + h,
                });
            }
        }
    }
    Ok(rows)
}

/// The transformation of the rows of harmonic index `harmonic` of a model's cyclic constraint (ADR 0386): the real
/// cos/sin pair `a_H = R (cos theta a_L - sin theta b_L)`, `b_H = R (sin theta a_L + cos theta b_L)`, `theta = 2 pi k / N`,
/// over the unknowns `x = (a, b)` (the sine part `b` at offset `n`, the number of mesh unknowns) for `0 < 2 k < N`, and
/// `a_H = -R a_L` over `a` alone for `2 k = N`. The prescribed rows and the linear constraints act on both parts (the same
/// constraint in every sector); every value must be zero. A rigid set acts on each part with its own six reference unknowns
/// (ADR 0395): after the (doubled) mesh unknowns, per rigid set in constraint order six for the cosine part and, for a
/// doubled harmonic, six for the sine part; [`Transformation::references`] lists the set once per part, the cosine part
/// first. `harmonic = 0` and a harmonic above `N / 2` are refused (`k = 0` is [`transformation`]).
/// [`Transformation::n_mesh`] is the length of the (doubled) mesh part of `x`.
pub fn harmonic_transformation(
    model: &SolidModel,
    prescribed: &[Prescribed],
    harmonic: usize,
) -> Result<Transformation> {
    let fixed = prescribed_equations(model, prescribed)?;
    let dofs = model.dof_map();
    let n = dofs.n_equations();
    let ci = crate::cyclic::the_cyclic_constraint(model)?;
    let bd = crate::cyclic::boundary(model, ci)?;
    let sectors = bd.sectors();
    if harmonic == 0 || 2 * harmonic > sectors {
        return Err(FemError::InvalidValue {
            key: "harmonic",
            reason: format!(
                "harmonic index {harmonic} of a rotor of {sectors} sectors: the harmonic transformation takes 1 to {} (0 is \
                 the transformation of the static rows)",
                sectors / 2
            ),
        });
    }
    let doubled = 2 * harmonic < sectors;
    let (c, s) = if doubled {
        let theta = 2.0 * std::f64::consts::PI * harmonic as f64 / sectors as f64;
        (theta.cos(), theta.sin())
    } else {
        (-1.0, 0.0)
    };
    let copies: &[usize] = if doubled { &[0, n] } else { &[0] };
    let n_mesh = n * copies.len();
    // The reference unknowns of the rigid sets, one block of six per part (ADR 0395).
    let mut references = Vec::new();
    let mut nn = n_mesh;
    for (i, con) in model.constraints().iter().enumerate() {
        if let Constraint::RigidSet { .. } = con {
            for _ in copies {
                references.push((i, nn));
                nn += REFERENCE_UNKNOWNS;
            }
        }
    }
    let mut rows: Vec<Row> = Vec::new();
    for &off in copies {
        for &(eq, v) in &fixed {
            let (node, component) = dofs.node_component(eq).expect("prescribed equation");
            if v != 0.0 {
                return Err(zero_values_only(
                    RowOrigin::Prescribed { node, component }.description(model),
                    v,
                ));
            }
            rows.push(Row {
                origin: RowOrigin::Prescribed { node, component },
                terms: vec![(off + eq, 1.0)],
                value: 0.0,
                designated: off + eq,
            });
        }
    }
    for (i, con) in model.constraints().iter().enumerate() {
        match con {
            Constraint::Linear { terms, value } => {
                let origin = RowOrigin::Linear { constraint: i };
                if *value != 0.0 {
                    return Err(zero_values_only(origin.description(model), *value));
                }
                let mut merged: Vec<(usize, f64)> = Vec::new();
                for t in terms {
                    let eq = dofs.equation(t.node, t.component).ok_or(FemError::MissingComponent {
                        role: "linear constraint",
                        node: t.node,
                        component: t.component,
                        available: dofs.components(t.node),
                    })?;
                    match merged.iter_mut().find(|m| m.0 == eq) {
                        Some(m) => m.1 += t.coefficient,
                        None => merged.push((eq, t.coefficient)),
                    }
                }
                let Some(&(designated, _)) = merged.iter().find(|m| m.1 != 0.0) else {
                    return Err(constraint_error(format!(
                        "{}: its coefficients cancel to zero",
                        origin.description(model)
                    )));
                };
                for &off in copies {
                    rows.push(Row {
                        origin,
                        terms: merged.iter().map(|&(e, a)| (off + e, a)).collect(),
                        value: 0.0,
                        designated: off + designated,
                    });
                }
            }
            Constraint::RigidSet { set, reference } => {
                let offsets: Vec<usize> = references.iter().filter(|r| r.0 == i).map(|r| r.1).collect();
                for (&off, &r0) in copies.iter().zip(&offsets) {
                    rows.extend(rigid_set_rows(model, i, set, *reference, off, r0)?);
                }
            }
            Constraint::Cyclic { .. } => {
                rows.extend(cyclic_rows(model, i, &bd, c, s, doubled.then_some(n))?);
            }
            _ => {}
        }
    }
    eliminate(model, n, nn, n_mesh, references, rows, doubled)
}

/// The rows `u_c - t_c - (w x d)_c = 0` of rigid set `constraint` (the rows of [`transformation`], ADR 0296) on the mesh
/// unknowns at offset `mesh_offset` and the reference unknowns from `reference_offset` (one part of a harmonic problem, ADR
/// 0395).
fn rigid_set_rows(
    model: &SolidModel,
    constraint: usize,
    set: &str,
    reference: Vec3,
    mesh_offset: usize,
    reference_offset: usize,
) -> Result<Vec<Row>> {
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let nodes = mesh.node_set(set).ok_or_else(|| FemError::UnknownSet {
        kind: SetKind::Node,
        name: set.to_string(),
    })?;
    let (t0, w0) = (reference_offset, reference_offset + 3);
    let mut rows = Vec::new();
    for &node in nodes {
        let x = mesh.nodes()[node];
        let dv = [x[0] - reference[0], x[1] - reference[1], x[2] - reference[2]];
        for comp in 0..3 {
            let eq = dofs.equation(node, comp).ok_or(FemError::MissingComponent {
                role: "rigid set",
                node,
                component: comp,
                available: dofs.components(node),
            })?;
            let (j, k) = ((comp + 1) % 3, (comp + 2) % 3);
            let mut terms = vec![(mesh_offset + eq, 1.0), (t0 + comp, -1.0)];
            if dv[k] != 0.0 {
                terms.push((w0 + j, -dv[k]));
            }
            if dv[j] != 0.0 {
                terms.push((w0 + k, dv[j]));
            }
            rows.push(Row {
                origin: RowOrigin::RigidSet {
                    constraint,
                    node,
                    component: comp,
                },
                terms,
                value: 0.0,
                designated: mesh_offset + eq,
            });
        }
    }
    Ok(rows)
}

/// Gauss-Jordan elimination of `rows` over `nn` unknowns (`n` mesh unknowns named by the model, the sine part of a doubled
/// harmonic problem after them when `sine`), giving `x = T q + g` with `n_mesh` the length of the mesh part (ADR 0296; the
/// tail of [`transformation`]).
fn eliminate(
    model: &SolidModel,
    n: usize,
    nn: usize,
    n_mesh: usize,
    references: Vec<(usize, usize)>,
    rows: Vec<Row>,
    sine: bool,
) -> Result<Transformation> {
    let mut el = Elimination {
        model,
        n_mesh: n,
        sine,
        deps: BTreeMap::new(),
        users: BTreeMap::new(),
        dropped: Vec::new(),
        constrained: vec![false; nn],
    };
    for row in rows {
        el.process(row)?;
    }
    let independent: Vec<usize> = (0..nn).filter(|i| !el.deps.contains_key(i)).collect();
    let mut q_of = vec![usize::MAX; nn];
    for (a, &i) in independent.iter().enumerate() {
        q_of[i] = a;
    }
    let mut g = vec![0.0; nn];
    let mut row_ptr = vec![0usize];
    let (mut col, mut val) = (Vec::new(), Vec::new());
    for p in 0..nn {
        match el.deps.get(&p) {
            Some(e) => {
                g[p] = e.constant;
                // Keys ascend in x, and q_of is increasing: the columns are sorted.
                for (&kk, &v) in &e.coef {
                    col.push(q_of[kk]);
                    val.push(v);
                }
            }
            None => {
                col.push(q_of[p]);
                val.push(1.0);
            }
        }
        row_ptr.push(col.len());
    }
    Ok(Transformation {
        t: CsrMatrix::rect(nn, independent.len(), row_ptr, col, val),
        g,
        independent,
        n_mesh,
        references,
        dropped: el.dropped,
        constrained: el.constrained,
    })
}

/// The stiffness of the model's elastic supports as triplets (row, column, value) over the mesh unknowns, in the order of
/// the supports, then the nodes or faces, then the entries (ADR 0296 D4).
pub fn support_triplets(model: &SolidModel) -> Result<Vec<(usize, usize, f64)>> {
    let dofs = model.dof_map();
    let mesh = model.mesh();
    let mut out = Vec::new();
    for s in model.supports() {
        match s {
            ElasticSupport::NodalSpring {
                set,
                component,
                stiffness,
            } => {
                let nodes = mesh.node_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: SetKind::Node,
                    name: set.clone(),
                })?;
                for &node in nodes {
                    let eq = dofs.equation(node, *component).ok_or(FemError::MissingComponent {
                        role: "support",
                        node,
                        component: *component,
                        available: dofs.components(node),
                    })?;
                    out.push((eq, eq, *stiffness));
                }
            }
            ElasticSupport::Foundation { set, stiffness } => {
                let faces = mesh.surface_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: SetKind::Surface,
                    name: set.clone(),
                })?;
                for &face in faces {
                    let nodes = mesh.face_nodes(face);
                    let x: Vec<Vec3> = nodes.iter().map(|&a| mesh.nodes()[a]).collect();
                    let m = nodes.len();
                    let mut ke = vec![0.0; 9 * m * m];
                    for (uv, w) in face_rule(m)? {
                        let (nf, df) = face_shape_functions(m, uv)?;
                        let mut xu = [0.0; 3];
                        let mut xv = [0.0; 3];
                        for (xa, da) in x.iter().zip(&df) {
                            for i in 0..3 {
                                xu[i] += xa[i] * da[0];
                                xv[i] += xa[i] * da[1];
                            }
                        }
                        let cv = cross(xu, xv);
                        // n n^T dA = w c c^T / |c|.
                        let s = w * stiffness / norm(cv);
                        for a in 0..m {
                            for bb in 0..m {
                                let nab = s * nf[a] * nf[bb];
                                for i in 0..3 {
                                    for j in 0..3 {
                                        ke[(3 * a + i) * 3 * m + 3 * bb + j] += nab * cv[i] * cv[j];
                                    }
                                }
                            }
                        }
                    }
                    for a in 0..m {
                        for bb in 0..m {
                            for i in 0..3 {
                                for j in 0..3 {
                                    let ea = dofs.equation(nodes[a], i).expect("face node component");
                                    let eb = dofs.equation(nodes[bb], j).expect("face node component");
                                    out.push((ea, eb, ke[(3 * a + i) * 3 * m + 3 * bb + j]));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Adds the supports' stiffness to `k` (on its pattern; an entry outside it is refused).
pub fn add_supports(k: &mut CsrMatrix, model: &SolidModel) -> Result<()> {
    for (i, j, v) in support_triplets(model)? {
        let row = &k.col[k.row_ptr[i]..k.row_ptr[i + 1]];
        let pos = row.binary_search(&j).map_err(|_| FemError::InvalidValue {
            key: "support",
            reason: format!("entry ({i}, {j}) is outside the stiffness pattern"),
        })?;
        k.val[k.row_ptr[i] + pos] += v;
    }
    Ok(())
}

/// The result of a static solve on the transformation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MpcSolution {
    /// Displacements of the mesh, reactions `rho` at the constrained mesh unknowns (0 elsewhere), stats.
    pub result: SolidResult,
    /// What the solver did.
    pub report: SolveReport,
    /// The transformation.
    pub transformation: Transformation,
    /// `K_r = T^T K_ext T`.
    pub reduced_matrix: CsrMatrix,
    /// The motion of each rigid set's reference point.
    pub references: Vec<ReferenceMotion>,
    /// `rho = K_ext x - f_ext` over all unknowns (mesh, then reference): the force of the constraints on the body.
    pub constraint_forces: Vec<f64>,
    /// `-K_s u`, the force of the elastic supports on the body, per mesh unknown.
    pub support_forces: Vec<f64>,
    /// The rows dropped as redundant.
    pub dropped: Vec<DroppedRow>,
}

/// The static solve on the transformation `u = T q + g` (ADR 0296), with per-node prescribed values and forces and forces
/// and moments at the reference points of rigid sets. Direct and Jacobi-CG solvers; the preconditioned CG is refused.
pub fn solve_static_mpc(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    reference_loads: &[ReferenceLoad],
    options: &StaticOptions,
) -> Result<MpcSolution> {
    model.validate()?;
    model.check_supported()?;
    if let StaticSolver::PreconditionedCg { .. } = options.solver {
        return Err(FemError::InvalidValue {
            key: "solver",
            reason: "the preconditioned CG is not available on the transformation path (ADR 0296): use Direct or \
                     ConjugateGradient"
                .into(),
        });
    }
    let dofs = model.dof_map();
    let tr = transformation(model, prescribed)?;
    let n = tr.n_mesh();
    let nn = tr.n_unknowns();
    let mut f = nodal_load_vector(model, forces)?;
    f.resize(nn, 0.0);
    for l in reference_loads {
        let r0 = tr
            .reference_offset(l.constraint)
            .ok_or_else(|| FemError::InvalidValue {
                key: "reference load",
                reason: format!("constraint {} is not a rigid set", l.constraint),
            })?;
        if l.force.iter().chain(&l.moment).any(|v| !v.is_finite()) {
            return Err(FemError::InvalidValue {
                key: "reference load",
                reason: format!("force {:?} or moment {:?} is not finite", l.force, l.moment),
            });
        }
        for i in 0..3 {
            f[r0 + i] += l.force[i];
            f[r0 + 3 + i] += l.moment[i];
        }
    }
    let mut k = stiffness_with(model, options.threads)?;
    add_supports(&mut k, model)?;
    let (kr, fr) = tr.reduce(&k, &f)?;
    let nq = kr.n;
    let (solver, q, iterations) = if nq == 0 {
        (solver_name(&options.solver), Vec::new(), 0)
    } else {
        match &options.solver {
            StaticSolver::ConjugateGradient {
                tolerance,
                max_iterations,
            } => {
                let (q, it) = conjugate_gradient(&kr, &fr, *tolerance, *max_iterations)?;
                ("cg-jacobi", q, it)
            }
            _ => ("skyline-ldlt", direct(&kr, &fr, &tr, model)?, 0),
        }
    };
    let relative_residual = residual_norm(&kr, &fr, &q);
    let x = tr.expand(&q);
    let u = x[..n].to_vec();
    let mut ku = vec![0.0; n];
    k.matvec(&u, &mut ku);
    let mut rho = vec![0.0; nn];
    for i in 0..nn {
        let kx = if i < n { ku[i] } else { 0.0 };
        rho[i] = kx - f[i];
    }
    let reactions: Vec<f64> = (0..n)
        .map(|i| if tr.is_constrained(i) { rho[i] } else { 0.0 })
        .collect();
    let mut support_forces = vec![0.0; n];
    for (i, j, v) in support_triplets(model)? {
        support_forces[i] -= v * u[j];
    }
    let references = tr
        .references()
        .iter()
        .map(|&(constraint, r0)| ReferenceMotion {
            constraint,
            translation: [x[r0], x[r0 + 1], x[r0 + 2]],
            rotation: [x[r0 + 3], x[r0 + 4], x[r0 + 5]],
        })
        .collect();
    let report = SolveReport {
        solver: solver.to_string(),
        iterations,
        restarts: 0,
        relative_residual,
        shift: None,
        breakdowns: Vec::new(),
    };
    let result = SolidResult::new(dofs, u)?
        .with_reactions(reactions)?
        .with_stats(SolverStats::new(solver, nq, iterations, relative_residual));
    let dropped = tr.dropped().to_vec();
    Ok(MpcSolution {
        result,
        report,
        transformation: tr,
        reduced_matrix: kr,
        references,
        constraint_forces: rho,
        support_forces,
        dropped,
    })
}

fn solver_name(s: &StaticSolver) -> &'static str {
    match s {
        StaticSolver::ConjugateGradient { .. } => "cg-jacobi",
        _ => "skyline-ldlt",
    }
}

fn singular(tr: &Transformation, model: &SolidModel, reduced: usize, pivot: f64, diagonal: f64) -> FemError {
    let i = tr.independent()[reduced];
    if i < tr.n_mesh() {
        let (node, component) = model.dof_map().node_component(i).expect("mesh unknown");
        return FemError::SingularStiffness {
            node,
            component,
            pivot,
            diagonal,
        };
    }
    let (constraint, r0) = tr
        .references()
        .iter()
        .rev()
        .find(|&&(_, o)| o <= i)
        .copied()
        .expect("reference unknown");
    let what = [
        "translation x",
        "translation y",
        "translation z",
        "rotation x",
        "rotation y",
        "rotation z",
    ][i - r0];
    FemError::Solver {
        solver: "skyline-ldlt",
        reason: format!(
            "singular reduced stiffness at the reference {what} of constraint {constraint} (RigidSet): pivot {pivot:e}, \
             diagonal {diagonal:e}"
        ),
    }
}

fn direct(a: &CsrMatrix, b: &[f64], tr: &Transformation, model: &SolidModel) -> Result<Vec<f64>> {
    let diag = a.diag();
    let ldlt = SkylineLdlt::factor(a).map_err(|e| match e {
        SkylineError::Pivot { row, pivot, .. } => singular(tr, model, row, pivot, diag[row]),
        other => FemError::Solver {
            solver: "skyline-ldlt",
            reason: other.to_string(),
        },
    })?;
    for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
        if d <= PIVOT_TOLERANCE * diag[row] {
            return Err(singular(tr, model, row, d, diag[row]));
        }
    }
    Ok(ldlt.solve(b))
}

fn conjugate_gradient(a: &CsrMatrix, b: &[f64], tolerance: f64, max_iterations: usize) -> Result<(Vec<f64>, usize)> {
    let mut x = vec![0.0; b.len()];
    let m = Jacobi::new(&a.diag());
    let control = SolveControl {
        tol_abs: 0.0,
        tol_rel: tolerance,
        max_iter: max_iterations,
    };
    let s = cg(a, b, &mut x, &m, &control);
    if !s.converged {
        return Err(FemError::Solver {
            solver: "cg-jacobi",
            reason: format!(
                "not converged in {} iterations (residual {:e} from {:e}, tolerance {tolerance:e} relative)",
                s.iterations, s.final_residual, s.initial_residual
            ),
        });
    }
    Ok((x, s.iterations))
}

fn residual_norm(a: &CsrMatrix, b: &[f64], x: &[f64]) -> f64 {
    let bn = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if bn == 0.0 || x.is_empty() {
        return 0.0;
    }
    let mut r = vec![0.0; b.len()];
    a.residual(b, x, &mut r);
    r.iter().map(|v| v * v).sum::<f64>().sqrt() / bn
}

/// The result of a modal analysis on the transformation (ADR 0384).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MpcModes {
    /// The modal result: eigenpairs, the mesh part of each expanded mode `T q`, the shift and the Sturm check.
    pub result: ModalResult,
    /// The transformation (`g = 0`).
    pub transformation: Transformation,
    /// `K_r = T^T K_ext T` (with the supports).
    pub reduced_stiffness: CsrMatrix,
    /// `M_r = T^T M_ext T`.
    pub reduced_mass: CsrMatrix,
    /// Each mode expanded over all `N` unknowns, `x = T q` (mesh, then the reference unknowns of the rigid sets).
    pub expanded: Vec<Vec<f64>>,
}

/// Pivot ratio `d_i / a_ii` below which the factor of `K_r` at zero shift is not accepted (the rule of `Shift::Auto`, ADR
/// 0283 D3).
const ZERO_SHIFT_PIVOT_RATIO: f64 = 1e-8;

/// A name of unknown `i` of the transformation: "node 4 component 1" or "the reference rotation y of constraint 3
/// (RigidSet)".
pub(crate) fn unknown_name(tr: &Transformation, model: &SolidModel, i: usize) -> String {
    if i < tr.n_mesh() {
        let (node, component) = model.dof_map().node_component(i).expect("mesh unknown");
        return format!("node {node} component {component}");
    }
    let (constraint, r0) = tr
        .references()
        .iter()
        .rev()
        .find(|&&(_, o)| o <= i)
        .copied()
        .expect("reference unknown");
    let what = [
        "translation x",
        "translation y",
        "translation z",
        "rotation x",
        "rotation y",
        "rotation z",
    ][i - r0];
    format!("the reference {what} of constraint {constraint} (RigidSet)")
}

/// Whether the factor of `k` at zero shift is refused or has a pivot ratio below the rule of `Shift::Auto`.
pub(crate) fn zero_shift_refused(k: &CsrMatrix) -> bool {
    match SkylineLdlt::factor_with(k, &SkylineOptions::default()) {
        Ok(f) => {
            let d = k.diag();
            f.pivots().iter().zip(f.order()).any(|(&p, &o)| {
                let ratio = p / d[o];
                ratio.is_nan() || ratio < ZERO_SHIFT_PIVOT_RATIO
            })
        }
        Err(_) => true,
    }
}

fn zero_values_only(what: String, v: f64) -> FemError {
    constraint_error(format!("{what}: value {v:e}; a modal analysis takes zero values only"))
}

/// The modal analysis on the transformation `x = T q` (ADR 0384): `K_r q = lambda M_r q` with `K_r = T^T K_ext T` (the
/// supports added) and `M_r = T^T M_ext T` (the reference unknowns carry no mass of their own), solved by
/// [`rcfd_linalg::eigen::lowest_eigenpairs`]; each mode is expanded, `x = T q`. Every constraint value must be zero (a
/// non-zero value is refused naming its row). The default shift is `Shift::Auto`, whose first candidate is zero: a model
/// whose constraints and supports hold every motion is solved at zero shift, a free motion moves it.
pub fn modal_analysis_mpc(model: &SolidModel, extra: &[Prescribed], options: &ModalOptions) -> Result<MpcModes> {
    model.validate()?;
    model.check_supported()?;
    let dofs = model.dof_map();
    for &(eq, v) in &prescribed_equations(model, extra)? {
        if v != 0.0 {
            let (node, component) = dofs.node_component(eq).expect("prescribed equation");
            return Err(zero_values_only(
                RowOrigin::Prescribed { node, component }.description(model),
                v,
            ));
        }
    }
    for (i, c) in model.constraints().iter().enumerate() {
        if let Constraint::Linear { value, .. } = c {
            if *value != 0.0 {
                return Err(zero_values_only(
                    RowOrigin::Linear { constraint: i }.description(model),
                    *value,
                ));
            }
        }
    }
    let tr = transformation(model, extra)?;
    let n = tr.n_mesh();
    let mut k = stiffness_with(model, options.threads)?;
    add_supports(&mut k, model)?;
    let m = consistent_with(model, options.threads)?;
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&k, &zero)?;
    let (mr, _) = tr.reduce(&m, &zero)?;
    let nq = kr.n;
    if options.modes == 0 || options.modes >= nq {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} modes requested of a model with {nq} free unknowns (at least 1 and fewer than the unknowns)",
                options.modes
            ),
        });
    }
    for (a, d) in mr.diag().iter().enumerate() {
        if d.is_nan() || *d <= 0.0 {
            return Err(FemError::InvalidValue {
                key: "density",
                reason: format!(
                    "{} has no mass (diagonal {d:e})",
                    unknown_name(&tr, model, tr.independent()[a])
                ),
            });
        }
    }
    let opts = SubspaceOptions {
        shift: options.shift.unwrap_or(Shift::Auto),
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&kr, &mr, options.modes, &opts).map_err(|e| FemError::Solver {
        solver: "lowest_eigenpairs",
        reason: e.to_string(),
    })?;
    let rigid_free = match options.shift {
        None => eig.shift != 0.0,
        Some(_) => zero_shift_refused(&kr),
    };
    let expanded: Vec<Vec<f64>> = eig.vectors.iter().map(|q| tr.expand(q)).collect();
    let modes = expanded.iter().map(|x| x[..n].to_vec()).collect();
    let result = ModalResult::from_parts(eig, modes, nq, rigid_free, dofs);
    Ok(MpcModes {
        result,
        transformation: tr,
        reduced_stiffness: kr,
        reduced_mass: mr,
        expanded,
    })
}

/// Refuses a non-zero constraint value for an analysis on the transformation that takes zero values only (ADR 0398 D3):
/// the prescribed rows first, then the `Linear` values; the reason names the row and says "{analysis} takes zero values
/// only".
pub(crate) fn refuse_nonzero_values(model: &SolidModel, analysis: &str) -> Result<()> {
    let dofs = model.dof_map();
    for &(eq, v) in &prescribed_equations(model, &[])? {
        if v != 0.0 {
            let (node, component) = dofs.node_component(eq).expect("prescribed equation");
            return Err(constraint_error(format!(
                "{}: value {v:e}; {analysis} takes zero values only",
                RowOrigin::Prescribed { node, component }.description(model)
            )));
        }
    }
    for (i, c) in model.constraints().iter().enumerate() {
        if let Constraint::Linear { value, .. } = c {
            if *value != 0.0 {
                return Err(constraint_error(format!(
                    "{}: value {value:e}; {analysis} takes zero values only",
                    RowOrigin::Linear { constraint: i }.description(model)
                )));
            }
        }
    }
    Ok(())
}

/// Refuses, for the modal analysis, a linear constraint, a rigid set or an elastic support (ADR 0296 D7). Since ADR 0384
/// the modal analysis solves these models on the transformation ([`modal_analysis_mpc`]) and no longer calls this function;
/// it is kept for callers that must refuse them.
pub fn refuse_in_modal(model: &SolidModel) -> Result<()> {
    const PACKAGE: &str = "a follow-up of ADR 0296 (modal analysis on the transformation)";
    for (index, c) in model.constraints().iter().enumerate() {
        let location = match c {
            Constraint::Linear { terms, .. } => {
                let first = terms.first().map_or(0, |t| t.node);
                format!("constraint {index}, first node {first}")
            }
            Constraint::RigidSet { set, .. } => format!("constraint {index}, node set '{set}'"),
            _ => continue,
        };
        return Err(FemError::NotYetSupported {
            item: "constraint",
            variant: c.name(),
            location,
            package: PACKAGE,
        });
    }
    if let Some((i, s)) = model.supports().iter().enumerate().next() {
        let location = match s {
            ElasticSupport::NodalSpring { set, .. } => format!("support {i}, node set '{set}'"),
            ElasticSupport::Foundation { set, .. } => format!("support {i}, surface set '{set}'"),
        };
        return Err(FemError::NotYetSupported {
            item: "support",
            variant: s.name(),
            location,
            package: PACKAGE,
        });
    }
    Ok(())
}
