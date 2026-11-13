//! Displacement constraints and the reduction of the system (ADR 0275 D3, D5). Declared by ADR 0271; filled by ADR 0275.
//!
//! Prescribed displacements come from the model's [`Constraint::Displacement`] (one value on every node of a node set)
//! and from per-node [`Prescribed`] values. They are imposed by symmetric elimination: the prescribed unknowns are
//! removed, `K_FF u_F = f_F - K_FP g`, and the reactions are `K u - f` at the prescribed unknowns. [`reduce`] is the one
//! function a transformation method `u = T q + g` replaces (ADR 0296).
//! Linear constraints and rigid sets give no prescribed equation here: [`crate::mpc`] handles them (and the elastic
//! supports) by the transformation, and [`crate::solve::solve_static_report`] dispatches to it.
//!
//! Before any factorisation, [`check_rigid_body_motions`] refuses a constraint set that leaves a rigid-body motion of a
//! connected part free: the six rigid motions `a + w x (x - c)` restricted to the prescribed components must have full
//! rank (ADR 0275 arithmetic 1 to 3).

use crate::error::{FemError, Result};
use crate::model::{Constraint, DofMap, SolidModel};
use rcfd_linalg::eigen::symmetric_eigen;
use rcfd_linalg::CsrMatrix;

/// Relative eigenvalue threshold of the rigid-motion Gram matrix (ADR 0275 arithmetic 2).
pub const RIGID_TOLERANCE: f64 = 1e-10;

/// A prescribed value of one component of the displacement at one node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prescribed {
    /// Node index.
    pub node: usize,
    /// Component (0, 1, 2 for x, y, z).
    pub component: usize,
    /// Prescribed value, m.
    pub value: f64,
}

impl Prescribed {
    /// A prescribed value.
    pub fn new(node: usize, component: usize, value: f64) -> Prescribed {
        Prescribed { node, component, value }
    }
}

fn pending_constraint(c: &Constraint, location: String, package: &'static str) -> FemError {
    FemError::NotYetSupported {
        item: "constraint",
        variant: c.name(),
        location,
        package,
    }
}

/// The prescribed equations of a model and of `extra`, as (equation, value) in ascending equation order. Refuses a
/// missing node or component (naming it), a non-finite value, and one equation prescribed with two different values;
/// bit-equal repeats are kept once.
pub fn prescribed_equations(model: &SolidModel, extra: &[Prescribed]) -> Result<Vec<(usize, f64)>> {
    let dofs = model.dof_map();
    let n_nodes = dofs.n_nodes();
    let mut list: Vec<(usize, f64)> = Vec::new();
    for (index, c) in model.constraints().iter().enumerate() {
        match c {
            Constraint::Displacement { set, component, value } => {
                let nodes = model.mesh().node_set(set).ok_or_else(|| FemError::UnknownSet {
                    kind: crate::error::SetKind::Node,
                    name: set.clone(),
                })?;
                for &node in nodes {
                    list.push((locate(&dofs, "constraint", node, *component)?, *value));
                }
            }
            // The constraints of the intermediate tier are refused until their package lands (ADR 0291 D6).
            Constraint::Linear { terms, .. } => {
                if let Some(package) = crate::mpc::PENDING {
                    let first = terms.first().map_or(0, |t| t.node);
                    return Err(pending_constraint(
                        c,
                        format!("constraint {index}, first node {first}"),
                        package,
                    ));
                }
            }
            Constraint::RigidSet { set, .. } => {
                if let Some(package) = crate::mpc::PENDING {
                    return Err(pending_constraint(
                        c,
                        format!("constraint {index}, node set '{set}'"),
                        package,
                    ));
                }
            }
            Constraint::Cyclic { low, high, .. } => {
                if let Some(package) = crate::cyclic::PENDING {
                    let location = format!("constraint {index}, surface sets '{low}' and '{high}'");
                    return Err(pending_constraint(c, location, package));
                }
            }
        }
    }
    for p in extra {
        if p.node >= n_nodes {
            return Err(FemError::MissingNode {
                role: "constraint",
                node: p.node,
                n_nodes,
            });
        }
        if !p.value.is_finite() {
            return Err(FemError::InvalidValue {
                key: "value",
                reason: format!("prescribed value {} at node {} is not finite", p.value, p.node),
            });
        }
        list.push((locate(&dofs, "constraint", p.node, p.component)?, p.value));
    }
    // Stable sort: the first value given for an equation comes first.
    list.sort_by_key(|&(eq, _)| eq);
    let mut out: Vec<(usize, f64)> = Vec::with_capacity(list.len());
    for (eq, v) in list {
        if let Some(&(last, first)) = out.last() {
            if last == eq {
                if first.to_bits() != v.to_bits() {
                    let (node, component) = dofs.node_component(eq).expect("existing equation");
                    return Err(FemError::ConflictingConstraint {
                        node,
                        component,
                        first,
                        second: v,
                    });
                }
                continue;
            }
        }
        out.push((eq, v));
    }
    Ok(out)
}

fn locate(dofs: &DofMap, role: &'static str, node: usize, component: usize) -> Result<usize> {
    if node >= dofs.n_nodes() {
        return Err(FemError::MissingNode {
            role,
            node,
            n_nodes: dofs.n_nodes(),
        });
    }
    dofs.equation(node, component).ok_or(FemError::MissingComponent {
        role,
        node,
        component,
        available: dofs.components(node),
    })
}

/// Connected parts of the mesh (nodes joined through elements): for each node the smallest node of its part, or `None`
/// for a node of no element.
pub fn parts(model: &SolidModel) -> Vec<Option<usize>> {
    let mesh = model.mesh();
    let n = mesh.nodes().len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut used = vec![false; n];
    fn find(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    for e in 0..mesh.n_elements() {
        let nodes = mesh.element_nodes(e);
        for &a in nodes {
            used[a] = true;
        }
        for &b in &nodes[1..] {
            let (ra, rb) = (find(&mut parent, nodes[0]), find(&mut parent, b));
            // The smaller root survives, so every root is the smallest node of its part.
            if ra != rb {
                parent[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    (0..n)
        .map(|a| if used[a] { Some(find(&mut parent, a)) } else { None })
        .collect()
}

/// Refuses a set of prescribed equations that leaves a rigid-body motion of a connected part, or a component of a node of
/// no element, free (ADR 0275 D5).
pub fn check_rigid_body_motions(model: &SolidModel, prescribed: &[(usize, f64)]) -> Result<()> {
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes();
    let part_of = parts(model);
    let mut fixed = vec![false; dofs.n_equations()];
    for &(eq, _) in prescribed {
        fixed[eq] = true;
    }
    // Nodes of no element: every component must be prescribed.
    for (node, p) in part_of.iter().enumerate() {
        if p.is_none() {
            for component in 0..dofs.components(node) {
                if !fixed[dofs.equation(node, component).expect("component")] {
                    return Err(FemError::FreeNode { node, component });
                }
            }
        }
    }
    // Parts in the order of their smallest node.
    let mut roots: Vec<usize> = part_of.iter().flatten().copied().collect();
    roots.sort_unstable();
    roots.dedup();
    for root in roots {
        let members: Vec<usize> = (0..nodes.len()).filter(|&a| part_of[a] == Some(root)).collect();
        let (c, l) = centre_and_radius(nodes, &members);
        let g = gram(&dofs, nodes, &members, &fixed, c, l);
        let eig = symmetric_eigen(&g, 6).map_err(|e| FemError::Solver {
            solver: "symmetric_eigen",
            reason: e.to_string(),
        })?;
        let lmax = eig.values[5];
        let free = eig
            .values
            .iter()
            .filter(|&&v| lmax <= 0.0 || v <= RIGID_TOLERANCE * lmax)
            .count();
        if free > 0 {
            let q = &eig.vectors[0];
            let motion = format!(
                "translation ({:.3}, {:.3}, {:.3}) with rotation ({:.3}, {:.3}, {:.3})/m about ({:.6e}, {:.6e}, {:.6e})",
                q[0],
                q[1],
                q[2],
                q[3] / l,
                q[4] / l,
                q[5] / l,
                c[0],
                c[1],
                c[2]
            );
            return Err(FemError::RigidBodyMotionFree {
                part_node: root,
                part_nodes: members.len(),
                count: free,
                motion,
            });
        }
    }
    Ok(())
}

/// The ratio `lambda_min / lambda_max` of the rigid-motion Gram matrix of each part (smallest node, ratio), for records.
pub fn rigid_gram_ratios(model: &SolidModel, prescribed: &[(usize, f64)]) -> Result<Vec<(usize, f64)>> {
    let dofs = model.dof_map();
    let nodes = model.mesh().nodes();
    let part_of = parts(model);
    let mut fixed = vec![false; dofs.n_equations()];
    for &(eq, _) in prescribed {
        fixed[eq] = true;
    }
    let mut roots: Vec<usize> = part_of.iter().flatten().copied().collect();
    roots.sort_unstable();
    roots.dedup();
    let mut out = Vec::new();
    for root in roots {
        let members: Vec<usize> = (0..nodes.len()).filter(|&a| part_of[a] == Some(root)).collect();
        let (c, l) = centre_and_radius(nodes, &members);
        let g = gram(&dofs, nodes, &members, &fixed, c, l);
        let eig = symmetric_eigen(&g, 6).map_err(|e| FemError::Solver {
            solver: "symmetric_eigen",
            reason: e.to_string(),
        })?;
        let lmax = eig.values[5];
        out.push((root, if lmax > 0.0 { eig.values[0] / lmax } else { 0.0 }));
    }
    Ok(out)
}

fn centre_and_radius(nodes: &[[f64; 3]], members: &[usize]) -> ([f64; 3], f64) {
    let m = members.len() as f64;
    let mut c = [0.0; 3];
    for &a in members {
        for k in 0..3 {
            c[k] += nodes[a][k];
        }
    }
    for ck in c.iter_mut() {
        *ck /= m;
    }
    let mut l: f64 = 0.0;
    for &a in members {
        let d = (0..3).map(|k| (nodes[a][k] - c[k]).powi(2)).sum::<f64>().sqrt();
        l = l.max(d);
    }
    (c, if l == 0.0 { 1.0 } else { l })
}

fn gram(dofs: &DofMap, nodes: &[[f64; 3]], members: &[usize], fixed: &[bool], c: [f64; 3], l: f64) -> [f64; 36] {
    let mut g = [0.0f64; 36];
    for &a in members {
        let y = [
            (nodes[a][0] - c[0]) / l,
            (nodes[a][1] - c[1]) / l,
            (nodes[a][2] - c[2]) / l,
        ];
        for comp in 0..dofs.components(a) {
            if !fixed[dofs.equation(a, comp).expect("component")] {
                continue;
            }
            let phi = rigid_row(comp, y);
            for r in 0..6 {
                for s in r..6 {
                    g[6 * r + s] += phi[r] * phi[s];
                }
            }
        }
    }
    for r in 0..6 {
        for s in 0..r {
            g[6 * r + s] = g[6 * s + r];
        }
    }
    g
}

/// The value of component `comp` of the rigid motion `q = (a, L w)` at the scaled position `y` (ADR 0275 arithmetic 1):
/// translations for components 0 to 2, the rotation itself for components 3 to 5 (none in the basic tier).
fn rigid_row(comp: usize, y: [f64; 3]) -> [f64; 6] {
    match comp {
        0 => [1.0, 0.0, 0.0, 0.0, y[2], -y[1]],
        1 => [0.0, 1.0, 0.0, -y[2], 0.0, y[0]],
        2 => [0.0, 0.0, 1.0, y[1], -y[0], 0.0],
        k => {
            let mut r = [0.0; 6];
            if k < 6 {
                r[k] = 1.0;
            }
            r
        }
    }
}

/// The system after the elimination of the prescribed unknowns.
#[derive(Debug, Clone, PartialEq)]
pub struct ReducedSystem {
    /// `K_FF`, the principal submatrix of the free unknowns (ascending).
    pub matrix: CsrMatrix,
    /// `f_F - K_FP g`.
    pub rhs: Vec<f64>,
    free: Vec<usize>,
    prescribed: Vec<(usize, f64)>,
    n: usize,
}

impl ReducedSystem {
    /// The free equations, ascending: reduced index `i` is full equation `free()[i]`.
    pub fn free(&self) -> &[usize] {
        &self.free
    }
    /// The prescribed (equation, value) pairs.
    pub fn prescribed(&self) -> &[(usize, f64)] {
        &self.prescribed
    }
    /// The full vector of unknowns from the free values: prescribed values are copied bit for bit.
    pub fn expand(&self, u_free: &[f64]) -> Vec<f64> {
        assert_eq!(u_free.len(), self.free.len(), "free solution length");
        let mut u = vec![0.0; self.n];
        for (&eq, &v) in self.free.iter().zip(u_free) {
            u[eq] = v;
        }
        for &(eq, v) in &self.prescribed {
            u[eq] = v;
        }
        u
    }
}

/// Symmetric elimination of the prescribed unknowns from `K u = f`: `K_FF u_F = f_F - K_FP g` (ADR 0275 D3).
/// `prescribed` is sorted by equation, without repeats (as [`prescribed_equations`] returns it).
pub fn reduce(k: &CsrMatrix, f: &[f64], prescribed: &[(usize, f64)]) -> Result<ReducedSystem> {
    let n = k.n;
    if f.len() != n {
        return Err(FemError::FieldLength {
            key: "load vector",
            expected: n,
            found: f.len(),
        });
    }
    let mut g = vec![0.0; n];
    let mut is_fixed = vec![false; n];
    for &(eq, v) in prescribed {
        is_fixed[eq] = true;
        g[eq] = v;
    }
    let free: Vec<usize> = (0..n).filter(|&i| !is_fixed[i]).collect();
    let mut reduced_index = vec![usize::MAX; n];
    for (r, &i) in free.iter().enumerate() {
        reduced_index[i] = r;
    }
    let mut row_ptr = Vec::with_capacity(free.len() + 1);
    row_ptr.push(0usize);
    let (mut col, mut val) = (Vec::new(), Vec::new());
    let mut rhs = Vec::with_capacity(free.len());
    for &i in &free {
        let mut b = f[i];
        for p in k.row_ptr[i]..k.row_ptr[i + 1] {
            let j = k.col[p];
            if is_fixed[j] {
                b -= k.val[p] * g[j];
            } else {
                col.push(reduced_index[j]);
                val.push(k.val[p]);
            }
        }
        rhs.push(b);
        row_ptr.push(col.len());
    }
    let nf = free.len();
    Ok(ReducedSystem {
        matrix: CsrMatrix::rect(nf, nf, row_ptr, col, val),
        rhs,
        free,
        prescribed: prescribed.to_vec(),
        n,
    })
}

/// Reactions `K u - f` at the prescribed equations, 0 at the free ones: the force the constraints exert on the body.
pub fn reactions(k: &CsrMatrix, u: &[f64], f: &[f64], prescribed: &[(usize, f64)]) -> Vec<f64> {
    let mut ku = vec![0.0; k.n];
    k.matvec(u, &mut ku);
    let mut r = vec![0.0; k.n];
    for &(eq, _) in prescribed {
        r[eq] = ku[eq] - f[eq];
    }
    r
}
