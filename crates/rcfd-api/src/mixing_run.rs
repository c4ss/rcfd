#![allow(clippy::neg_cmp_op_on_partial_ord)]
//! Mixing plane in the steady solver loop (F-126, ADR 0059 D10-D12).
//!
//! A `mixing_plane` section couples two patches of one mesh, an upstream (outlet) patch of a frozen-rotor zone and a
//! downstream (inlet) patch of a stationary zone, that are NOT connected by faces (generator `annulus_3d_stage`). Before every
//! `every`-th iteration the circumferentially averaged state of the one side is passed to the other with
//! [`rcfd_fv::mixing_plane::MixingPlane`], through under-relaxed per-face boundary conditions:
//!
//! * forward, upstream to downstream: the donor faces are the upstream outlet faces with the face velocity `u_cell` whose normal
//!   component is replaced so that `u . S = phi_f` (the solver flux); the passed absolute velocity is installed on the
//!   downstream inlet patch as a per-face fixed value, `u <- u + relax (u_passed - u)`;
//! * backward, downstream to upstream: the donor faces are the downstream inlet faces (installed velocity, extrapolated
//!   pressure), and only the passed mixed-out pressure is installed on the upstream outlet patch as a per-face fixed value,
//!   `p <- p + relax (p_passed - p)`. The mixed-out pressure is not imposed forward.
//!
//! At convergence the installed state equals the passed one, so mass and angular momentum flux through the two sides agree
//! band by band. The monitor `res_mixing` is the relative change of the exchange, `max(sum |u_passed - u| |S| / sum |u_passed|
//! |S|, sum |p_passed - p| |S| / (1/2 sum |u_passed|^2 |S|))`; the run converges only if at least two exchanges were made and the
//! change is below `mixing_plane.tolerance`. A band whose flux cancels is a failure with the message prefix `mixing_plane:`.

use crate::case::{Case, Diagnostic};
use crate::sim::{get_b, get_f, get_i, get_s, get_v};
use rcfd_core::settings::Schema;
use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::mixing_plane::{DonorFace, MixingPlane, ReceiverFace};
use rcfd_fv::pressure::{boundary_values, pressure_bc};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};

/// Sentinel of the numeric keys that are not set.
pub const UNSET: f64 = -1.0;

fn d(path: impl Into<String>, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: path.into(),
        line: None,
        message: message.into(),
    }
}

/// Is the mixing plane enabled?
pub fn enabled(case: &Case) -> bool {
    get_b(case, "mixing_plane.enabled")
}

/// Schema of the `mixing_plane` section.
pub fn schema() -> Schema {
    Schema::object()
        .doc("Mixing plane (stage interface, F-126, ADR 0059) between two patches of one mesh that are not connected (generator annulus_3d_stage): a frozen-rotor zone upstream, a stationary zone downstream; steady simple / simplec only")
        .field("enabled", Schema::boolean(false).doc("Couple the two patches by circumferential averaging in radial bands"))
        .field("upstream_patch", Schema::string("").doc("Outlet patch of the upstream zone: velocity = zero_gradient, pressure = fixed (the pressure is set by the plane); must be given"))
        .field("downstream_patch", Schema::string("").doc("Inlet patch of the downstream zone: velocity = fixed (set by the plane); must be given"))
        .field("axis", Schema::list(Schema::float(0.0)).doc("Rotation axis direction (3 components, default [0, 0, 1])"))
        .field("origin", Schema::list(Schema::float(0.0)).doc("Point on the axis [m] (3 components, default [0, 0, 0])"))
        .field("bands", Schema::int(-1).doc("Number of radial bands (>= 1); -1 = unset, an error. The bands must align with radial cell layers of both sides"))
        .field("r_min", Schema::float(UNSET).doc("Inner radius of the bands [m]; -1 = take the smaller extent of the two patches"))
        .field("r_max", Schema::float(UNSET).doc("Outer radius of the bands [m]; -1 = take the larger extent of the two patches"))
        .field("relax", Schema::float(0.5).doc("Under-relaxation of the passed velocity and pressure, 0 < relax <= 1"))
        .field("every", Schema::int(1).at_least(1.0).doc("Update the exchange every this many iterations (>= 1)"))
        .field("tolerance", Schema::float(1e-6).doc("The run counts as converged only if the relative change of the exchange is below this (> 0)"))
}

/// Settings checks that need no mesh.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !enabled(case) {
        return out;
    }
    let up = get_s(case, "mixing_plane.upstream_patch");
    let dn = get_s(case, "mixing_plane.downstream_patch");
    if up.is_empty() {
        out.push(d("mixing_plane.upstream_patch", "the upstream patch must be given"));
    }
    if dn.is_empty() {
        out.push(d("mixing_plane.downstream_patch", "the downstream patch must be given"));
    }
    if !up.is_empty() && up == dn {
        out.push(d(
            "mixing_plane.downstream_patch",
            "the upstream and downstream patches must differ",
        ));
    }
    let alg = get_s(case, "solver.algorithm");
    if !matches!(alg.as_str(), "simple" | "simplec") {
        out.push(d(
            "mixing_plane.enabled",
            format!("the mixing plane is available with the steady simple and simplec algorithms, not \"{alg}\""),
        ));
    }
    if get_b(case, "solver.frozen_flow") {
        out.push(d(
            "mixing_plane.enabled",
            "the mixing plane needs a solved flow; it is not available with solver.frozen_flow",
        ));
    }
    if get_i(case, "mixing_plane.bands") < 1 {
        out.push(d(
            "mixing_plane.bands",
            "the number of radial bands must be set (>= 1); -1 means unset",
        ));
    }
    let (r0, r1) = (get_f(case, "mixing_plane.r_min"), get_f(case, "mixing_plane.r_max"));
    if (r0 == UNSET) != (r1 == UNSET) {
        out.push(d("mixing_plane.r_max", "give both r_min and r_max, or neither"));
    } else if r0 != UNSET && !(r0 >= 0.0 && r1 > r0) {
        out.push(d(
            "mixing_plane.r_max",
            format!("r_max {r1} must exceed r_min {r0} >= 0"),
        ));
    }
    for (key, def) in [("axis", [0.0, 0.0, 1.0]), ("origin", [0.0; 3])] {
        let p = format!("mixing_plane.{key}");
        let n = case
            .tree
            .get(&p)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.len()))
            .unwrap_or(0);
        if n != 0 && n != 3 {
            out.push(d(&p, format!("expected 3 components [x, y, z], got {n}")));
        } else {
            let v = get_v(case, &p, def);
            if !v.iter().all(|x| x.is_finite()) || (key == "axis" && vec3::norm(v) == 0.0) {
                out.push(d(
                    &p,
                    if key == "axis" {
                        "the axis must have non-zero length"
                    } else {
                        "the origin must be finite"
                    },
                ));
            }
        }
    }
    let relax = get_f(case, "mixing_plane.relax");
    if !(relax > 0.0 && relax <= 1.0) {
        out.push(d("mixing_plane.relax", "the relaxation must be in (0, 1]"));
    }
    if get_i(case, "mixing_plane.every") < 1 {
        out.push(d("mixing_plane.every", "the update interval must be at least 1"));
    }
    if !(get_f(case, "mixing_plane.tolerance") > 0.0) {
        out.push(d("mixing_plane.tolerance", "the tolerance must be positive"));
    }
    for (name, role) in [(&up, "upstream"), (&dn, "downstream")] {
        if name.is_empty() || !case.tree.names("boundaries").unwrap_or_default().contains(name) {
            continue;
        }
        let b = format!("boundaries.{name}");
        {
            let kind = get_s(case, &format!("{b}.bc_kind"));
            if !kind.is_empty() && kind != "none" {
                out.push(d(
                    format!("{b}.bc_kind"),
                    format!("the {role} mixing-plane patch '{name}' must not have a bc_kind (\"{kind}\"); the plane sets its conditions"),
                ));
            }
        }
        let vel = get_s(case, &format!("{b}.velocity"));
        if role == "upstream" {
            if vel != "zero_gradient" {
                out.push(d(
                    format!("{b}.velocity"),
                    format!(
                        "the upstream mixing-plane patch '{name}' needs velocity = \"zero_gradient\", got \"{vel}\""
                    ),
                ));
            }
            let pr = get_s(case, &format!("{b}.pressure"));
            if pr != "fixed" {
                out.push(d(
                    format!("{b}.pressure"),
                    format!("the upstream mixing-plane patch '{name}' needs pressure = \"fixed\" (the plane sets the value), got \"{pr}\""),
                ));
            }
        } else if vel != "fixed" {
            out.push(d(
                format!("{b}.velocity"),
                format!("the downstream mixing-plane patch '{name}' needs velocity = \"fixed\" (the plane sets the value), got \"{vel}\""),
            ));
        }
    }
    out
}

/// State of the mixing plane of a running simulation.
pub struct MixingRun {
    plane: MixingPlane,
    up_patch: String,
    dn_patch: String,
    up_faces: Vec<usize>,
    dn_faces: Vec<usize>,
    relax: f64,
    every: usize,
    tolerance: f64,
    u_dn: Vec<Vec3>,
    p_up: Vec<f64>,
    change: f64,
    exchanges: usize,
}

fn radius(origin: Vec3, axis: Vec3, x: Vec3) -> f64 {
    let a = vec3::scale(axis, 1.0 / vec3::norm(axis));
    let r = vec3::sub(x, origin);
    vec3::norm(vec3::sub(r, vec3::scale(a, vec3::dot(r, a))))
}

impl MixingRun {
    /// Build the exchange on the mesh of `geom`; the mesh-dependent problems are located diagnostics.
    pub fn new(case: &Case, geom: &FvGeometry) -> Result<MixingRun, Vec<Diagnostic>> {
        let errs = check(case);
        if !errs.is_empty() {
            return Err(errs);
        }
        let mesh = &geom.mesh;
        let mut errs = Vec::new();
        let mut faces = |key: &str| -> Vec<usize> {
            let name = get_s(case, key);
            match mesh.patch(&name) {
                Some(p) => p.faces().collect(),
                None => {
                    errs.push(d(key, format!("unknown boundary patch '{name}'")));
                    Vec::new()
                }
            }
        };
        let up_faces = faces("mixing_plane.upstream_patch");
        let dn_faces = faces("mixing_plane.downstream_patch");
        if errs.is_empty() {
            let listed = case.tree.names("boundaries").unwrap_or_default();
            for key in ["mixing_plane.upstream_patch", "mixing_plane.downstream_patch"] {
                let name = get_s(case, key);
                if !listed.contains(&name) {
                    errs.push(d(
                        format!("boundaries.{name}"),
                        format!("the mixing-plane patch '{name}' ({key}) needs a [boundaries.{name}] section (the plane sets its conditions)"),
                    ));
                }
            }
        }
        if !errs.is_empty() {
            return Err(errs);
        }
        let axis = get_v(case, "mixing_plane.axis", [0.0, 0.0, 1.0]);
        let origin = get_v(case, "mixing_plane.origin", [0.0; 3]);
        let (mut r0, mut r1) = (get_f(case, "mixing_plane.r_min"), get_f(case, "mixing_plane.r_max"));
        if r0 == UNSET {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for &f in up_faces.iter().chain(&dn_faces) {
                for &n in mesh.face_points(f) {
                    let r = radius(origin, axis, mesh.points()[n]);
                    lo = lo.min(r);
                    hi = hi.max(r);
                }
            }
            (r0, r1) = (lo, hi);
        }
        let bands = get_i(case, "mixing_plane.bands") as usize;
        let plane = MixingPlane::uniform(origin, axis, r0, r1, bands)
            .map_err(|e| vec![d("mixing_plane.bands", format!("mixing_plane: {e}"))])?;
        Ok(MixingRun {
            plane,
            up_patch: get_s(case, "mixing_plane.upstream_patch"),
            dn_patch: get_s(case, "mixing_plane.downstream_patch"),
            up_faces,
            dn_faces,
            relax: get_f(case, "mixing_plane.relax"),
            every: get_i(case, "mixing_plane.every").max(1) as usize,
            tolerance: get_f(case, "mixing_plane.tolerance"),
            u_dn: Vec::new(),
            p_up: Vec::new(),
            change: 1.0,
            exchanges: 0,
        })
    }

    /// Is an exchange due before the iteration after `completed` finished iterations?
    pub fn due(&self, completed: usize) -> bool {
        completed % self.every == 0
    }

    /// Update both patches from the current fields. Errors carry the prefix `mixing_plane:`.
    pub fn exchange(
        &mut self,
        geom: &FvGeometry,
        u: &mut VectorField,
        p: &mut ScalarField,
        phi: &[f64],
    ) -> Result<(), String> {
        let mesh = &geom.mesh;
        let (sf, fc, own, ni) = (
            mesh.face_area_vectors(),
            mesh.face_centres(),
            mesh.owner(),
            geom.n_internal,
        );
        let wrap = |e: rcfd_core::Error| format!("mixing_plane: {e}");
        if self.u_dn.is_empty() {
            let ub = u.boundary_values(geom);
            self.u_dn = self.dn_faces.iter().map(|&f| ub[f - ni]).collect();
            let pb = p.boundary_values(geom);
            self.p_up = self.up_faces.iter().map(|&f| pb[f - ni]).collect();
        }
        // forward: the upstream outlet faces carry the solver flux
        let donors: Vec<DonorFace> = self
            .up_faces
            .iter()
            .map(|&f| {
                let (uc, s) = (u.values[own[f]], sf[f]);
                let k = (phi[f] - vec3::dot(uc, s)) / vec3::dot(s, s);
                DonorFace {
                    s,
                    x: fc[f],
                    u: vec3::add(uc, vec3::scale(s, k)),
                    p: 0.0,
                    h: 0.0,
                }
            })
            .collect();
        let bands = self.plane.average(&donors).map_err(wrap)?;
        let recv: Vec<ReceiverFace> = self
            .dn_faces
            .iter()
            .map(|&f| ReceiverFace {
                s: vec3::scale(sf[f], -1.0),
                x: fc[f],
            })
            .collect();
        let passed = self.plane.pass(&bands, &recv).map_err(wrap)?;
        let (mut num, mut den) = (0.0, 0.0);
        for (k, &f) in self.dn_faces.iter().enumerate() {
            let s = vec3::norm(sf[f]);
            num += vec3::norm(vec3::sub(passed[k].u, self.u_dn[k])) * s;
            den += vec3::norm(passed[k].u) * s;
            self.u_dn[k] = vec3::add(
                self.u_dn[k],
                vec3::scale(vec3::sub(passed[k].u, self.u_dn[k]), self.relax),
            );
        }
        let change_u = num / den.max(1e-300);
        u.set_bc(
            geom,
            &self.dn_patch,
            Box::new(FixedValue(Profile::PerFace(self.u_dn.clone()))),
        )
        .map_err(wrap)?;
        // backward: the downstream inlet faces (installed velocity, extrapolated pressure) give the pressure of the outlet
        let pbc = pressure_bc(geom, u, p);
        let pb = boundary_values(geom, &pbc.coeffs, &p.values);
        let donors: Vec<DonorFace> = self
            .dn_faces
            .iter()
            .enumerate()
            .map(|(k, &f)| DonorFace {
                s: vec3::scale(sf[f], -1.0),
                x: fc[f],
                u: self.u_dn[k],
                p: pb[f - ni],
                h: 0.0,
            })
            .collect();
        let bands = self.plane.average(&donors).map_err(wrap)?;
        let recv: Vec<ReceiverFace> = self
            .up_faces
            .iter()
            .map(|&f| ReceiverFace { s: sf[f], x: fc[f] })
            .collect();
        let passed = self.plane.pass(&bands, &recv).map_err(wrap)?;
        let (mut num, mut den) = (0.0, 0.0);
        for (k, &f) in self.up_faces.iter().enumerate() {
            let s = vec3::norm(sf[f]);
            num += (passed[k].p - self.p_up[k]).abs() * s;
            den += 0.5 * vec3::dot(passed[k].u, passed[k].u) * s;
            self.p_up[k] += self.relax * (passed[k].p - self.p_up[k]);
        }
        let change_p = num / den.max(1e-300);
        let prof: Vec<Vec3> = self.p_up.iter().map(|&v| [v, 0.0, 0.0]).collect();
        p.set_bc(geom, &self.up_patch, Box::new(FixedValue(Profile::PerFace(prof))))
            .map_err(wrap)?;
        self.change = change_u.max(change_p);
        self.exchanges += 1;
        Ok(())
    }

    /// Relative change of the exchange at the last update (the monitor `res_mixing`).
    pub fn change(&self) -> f64 {
        self.change
    }

    /// Number of exchanges made.
    pub fn exchanges(&self) -> usize {
        self.exchanges
    }

    /// Converged: at least two exchanges and a change below the tolerance.
    pub fn converged(&self) -> bool {
        self.exchanges >= 2 && self.change < self.tolerance
    }

    /// The bands.
    pub fn plane(&self) -> &MixingPlane {
        &self.plane
    }

    /// Faces of the upstream patch.
    pub fn up_faces(&self) -> &[usize] {
        &self.up_faces
    }

    /// Faces of the downstream patch.
    pub fn dn_faces(&self) -> &[usize] {
        &self.dn_faces
    }
}
