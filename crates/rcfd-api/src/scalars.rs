//! Passive scalars and multicomponent species (F-053): `scalars.<name>` and `species.<name>`.
//!
//! Both are transported with the shared steady scalar routine of `rcfd_fv::scalar` on the flux of the
//! flow solver (`div(phi s) - div(D grad s) = S`, constant density). Species are mass fractions: all
//! but one are transported and the balance species (`remainder = true`, or the last one by name) is
//! closed by difference `Y_N = 1 - sum Y_i`, so the fractions sum to one to round-off and each
//! transported species is conserved separately (see `rcfd_fv::scalar::boundary_flux`).
//!
//! The diffusivity is either given (`diffusivity`, m^2/s) or `nu / Sc` (`schmidt`), which follows a
//! temperature-dependent or non-Newtonian face viscosity when the flow solver uses one.

use crate::case::{Case, Diagnostic};
use crate::exprs;
use rcfd_core::{Dimension, Error, Result};
use rcfd_fv::bc::{Bc, FixedGradient, FixedValue, Profile, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::scalar::{PatchFlux, ScalarTransport};
use rcfd_fv::{FvGeometry, ScalarField};
use std::sync::Arc;

/// Names that cannot be used for a scalar or species because they denote other fields.
const RESERVED: &[&str] = &[
    "ux",
    "uy",
    "uz",
    "speed",
    "p",
    "T",
    "U",
    "k",
    "epsilon",
    "omega",
    "nut",
    "rho",
    "mach",
    "y_plus",
    "wall_distance",
    "vorticity_z",
];

/// Name of the balance species (`remainder = true`, else the last species by name).
fn balance_of(case: &Case) -> String {
    let names = section_names(case, "species");
    names
        .iter()
        .find(|n| gb(case, &format!("species.{n}.remainder")))
        .cloned()
        .or_else(|| names.last().cloned())
        .unwrap_or_default()
}

const SECTIONS: [&str; 2] = ["scalars", "species"];

fn gf(c: &Case, p: &str) -> f64 {
    c.tree.get(p).ok().and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn gs(c: &Case, p: &str) -> String {
    c.tree
        .get(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn gb(c: &Case, p: &str) -> bool {
    c.tree.get(p).ok().and_then(|v| v.as_bool()).unwrap_or(false)
}

fn section_names(case: &Case, section: &str) -> Vec<String> {
    case.tree.names(section).unwrap_or_default()
}

/// Is any passive scalar or species configured?
pub fn enabled(case: &Case) -> bool {
    SECTIONS.iter().any(|s| !section_names(case, s).is_empty())
}

/// Settings checks.
pub fn check(case: &Case) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut bad = |path: String, message: String| {
        out.push(Diagnostic {
            path,
            line: None,
            message,
        })
    };
    if !enabled(case) {
        return out;
    }
    let alg = gs(case, "solver.algorithm");
    let steady = matches!(alg.as_str(), "simple" | "simplec" | "coupled");
    let nu = gf(case, "physics.nu");
    let mut seen: Vec<String> = Vec::new();
    for section in SECTIONS {
        let names = section_names(case, section);
        for n in &names {
            let p = |f: &str| format!("{section}.{n}.{f}");
            if !steady {
                bad(
                    format!("{section}.{n}"),
                    format!("scalar transport is available with the steady algorithms (simple, simplec, coupled), not with \"{alg}\""),
                );
            }
            if RESERVED.contains(&n.as_str()) || seen.contains(n) {
                bad(
                    format!("{section}.{n}"),
                    format!("the name '{n}' is already used by another field, scalar or species"),
                );
            }
            seen.push(n.clone());
            let (d, sc) = (gf(case, &p("diffusivity")), gf(case, &p("schmidt")));
            if section == "species" && *n == balance_of(case) {
                // the balance species is closed by difference and needs no transport data
                continue;
            }
            if d > 0.0 && sc > 0.0 {
                bad(p("schmidt"), "give either diffusivity or schmidt, not both".into());
            } else if d <= 0.0 && sc <= 0.0 {
                bad(
                    p("diffusivity"),
                    "needs a positive diffusivity [m^2/s] or a positive schmidt number".into(),
                );
            } else if sc > 0.0 && nu <= 0.0 {
                bad(p("schmidt"), "a Schmidt number needs a positive physics.nu".into());
            }
            if d < 0.0 {
                bad(p("diffusivity"), "must not be negative".into());
            }
            if sc < 0.0 {
                bad(p("schmidt"), "must not be negative".into());
            }
            let relax = gf(case, &p("relax"));
            if !(relax > 0.0 && relax <= 1.0) {
                bad(p("relax"), "the relaxation factor must be in (0, 1]".into());
            }
            for b in case.tree.names(&p("boundaries")).unwrap_or_default() {
                let t = gs(case, &p(&format!("boundaries.{b}.type")));
                if !matches!(t.as_str(), "zero_gradient" | "fixed" | "gradient") {
                    bad(
                        p(&format!("boundaries.{b}.type")),
                        format!("unknown boundary type '{t}' (zero_gradient, fixed, gradient)"),
                    );
                }
            }
        }
    }
    // species closure
    let names = section_names(case, "species");
    if !names.is_empty() {
        if names.len() < 2 {
            bad(
                format!("species.{}", names[0]),
                "a multicomponent set needs at least two species (the last one is closed by difference)".into(),
            );
        }
        let flagged: Vec<&String> = names
            .iter()
            .filter(|n| gb(case, &format!("species.{n}.remainder")))
            .collect();
        if flagged.len() > 1 {
            bad(
                format!("species.{}.remainder", flagged[1]),
                "only one species can be the balance species".into(),
            );
        }
        let balance = flagged
            .first()
            .map(|s| s.to_string())
            .unwrap_or_else(|| names.last().cloned().unwrap_or_default());
        let transported: Vec<&String> = names.iter().filter(|n| **n != balance).collect();
        for b in case
            .tree
            .names(&format!("species.{balance}.boundaries"))
            .unwrap_or_default()
        {
            bad(
                format!("species.{balance}.boundaries.{b}"),
                "the balance species is closed by difference; set the boundary conditions of the other species".into(),
            );
        }
        let init: f64 = names.iter().map(|n| gf(case, &format!("species.{n}.initial"))).sum();
        if (init - 1.0).abs() > 1e-9 {
            bad(
                format!("species.{balance}.initial"),
                format!("the initial mass fractions sum to {init}, not 1"),
            );
        }
        // patches where every transported species is fixed: the balance species is 1 - sum
        let mut patches: Vec<String> = Vec::new();
        for t in &transported {
            for b in case.tree.names(&format!("species.{t}.boundaries")).unwrap_or_default() {
                if !patches.contains(&b) {
                    patches.push(b);
                }
            }
        }
        for b in patches {
            let fixed: Vec<f64> = transported
                .iter()
                .filter(|t| gs(case, &format!("species.{t}.boundaries.{b}.type")) == "fixed")
                .map(|t| gf(case, &format!("species.{t}.boundaries.{b}.value")))
                .collect();
            let has_expr = transported.iter().any(|t| {
                !gs(case, &format!("species.{t}.boundaries.{b}.value_expression"))
                    .trim()
                    .is_empty()
            });
            if fixed.len() == transported.len() {
                if has_expr {
                    continue;
                }
                let s: f64 = fixed.iter().sum();
                if !(-1e-12..=1.0 + 1e-12).contains(&s) {
                    bad(
                        format!("species.{}.boundaries.{b}.value", transported[0]),
                        format!("the fixed mass fractions on '{b}' sum to {s}; the balance species would be negative or exceed one"),
                    );
                }
            } else if !fixed.is_empty() {
                bad(
                    format!("species.{}.boundaries.{b}.type", transported[0]),
                    format!("on '{b}' some species are fixed and others are not; the closure sum Y = 1 needs all of them prescribed alike"),
                );
            }
        }
    }
    out
}

struct Entry {
    name: String,
    transport: ScalarTransport,
    /// Schmidt number when the diffusivity follows the viscosity.
    schmidt: f64,
    res: f64,
}

/// Configured scalars and species.
pub struct ScalarState {
    passive: Vec<Entry>,
    species: Vec<Entry>,
    /// Name of the balance species (empty without species).
    balance: String,
}

fn boundary_condition(
    case: &Case,
    geom: &FvGeometry,
    patch: &str,
    path: &str,
    errs: &mut Vec<Diagnostic>,
) -> Box<dyn Bc> {
    let v = gf(case, &format!("{path}.value"));
    match gs(case, &format!("{path}.type")).as_str() {
        "fixed" => {
            let ctx = exprs::spatial_context(case);
            let expr_path = format!("{path}.value_expression");
            match exprs::compile_bc(case, &expr_path, Dimension::NONE, &ctx) {
                Ok(Some(c)) => {
                    let Some(p) = geom.mesh.patch(patch) else {
                        return Box::new(ZeroGradient);
                    };
                    let cf = geom.mesh.face_centres();
                    let vals: Vec<[f64; 3]> = p
                        .faces()
                        .map(|f| [c.eval(&[cf[f][0], cf[f][1], cf[f][2], 0.0]), 0.0, 0.0])
                        .collect();
                    if vals.iter().any(|x| !x[0].is_finite()) {
                        errs.push(Diagnostic {
                            path: expr_path,
                            line: None,
                            message: exprs::NONFINITE_BC.into(),
                        });
                    }
                    Box::new(FixedValue(Profile::PerFace(vals)))
                }
                Ok(None) => Box::new(FixedValue::scalar(v)),
                Err(e) => {
                    errs.extend(e);
                    Box::new(ZeroGradient)
                }
            }
        }
        "gradient" => Box::new(FixedGradient(Profile::scalar(v))),
        _ => Box::new(ZeroGradient),
    }
}

fn make(
    case: &Case,
    geom: &Arc<FvGeometry>,
    section: &str,
    name: &str,
    scheme: ConvScheme,
    nu: f64,
    with_bcs: bool,
    errs: &mut Vec<Diagnostic>,
) -> Entry {
    let p = |f: &str| format!("{section}.{name}.{f}");
    let mut field = ScalarField::uniform(name, geom, gf(case, &p("initial")));
    if with_bcs {
        for b in case.tree.names(&p("boundaries")).unwrap_or_default() {
            if geom.mesh.patch(&b).is_none() {
                let known: Vec<&str> = geom.mesh.patches().iter().map(|q| q.name.as_str()).collect();
                errs.push(Diagnostic {
                    path: p(&format!("boundaries.{b}")),
                    line: None,
                    message: format!("mesh has no patch '{b}' (patches: {})", known.join(", ")),
                });
                continue;
            }
            let bc = boundary_condition(case, geom, &b, &p(&format!("boundaries.{b}")), errs);
            let _ = field.set_bc(geom, &b, bc);
        }
    }
    let schmidt = gf(case, &p("schmidt"));
    let d = if schmidt > 0.0 {
        nu / schmidt
    } else {
        gf(case, &p("diffusivity"))
    };
    let mut t = ScalarTransport::new(geom.clone(), field, d);
    t.convection = scheme;
    t.relax = gf(case, &p("relax"));
    let src = gf(case, &p("source"));
    if src != 0.0 {
        t.source = Some(vec![src; geom.n_cells()]);
    }
    Entry {
        name: name.to_string(),
        transport: t,
        schmidt,
        res: 1.0,
    }
}

impl ScalarState {
    /// Build from the case; `None` when nothing is configured.
    pub fn from_case(
        case: &Case,
        geom: &Arc<FvGeometry>,
        scheme: ConvScheme,
        nu: f64,
    ) -> std::result::Result<Option<ScalarState>, Vec<Diagnostic>> {
        if !enabled(case) {
            return Ok(None);
        }
        let mut errs = Vec::new();
        let passive: Vec<Entry> = section_names(case, "scalars")
            .iter()
            .map(|n| make(case, geom, "scalars", n, scheme, nu, true, &mut errs))
            .collect();
        let names = section_names(case, "species");
        let balance = names
            .iter()
            .find(|n| gb(case, &format!("species.{n}.remainder")))
            .cloned()
            .or_else(|| names.last().cloned())
            .unwrap_or_default();
        let species: Vec<Entry> = names
            .iter()
            .filter(|n| **n != balance)
            .map(|n| make(case, geom, "species", n, scheme, nu, true, &mut errs))
            .collect();
        if !errs.is_empty() {
            return Err(errs);
        }
        Ok(Some(ScalarState {
            passive,
            species,
            balance,
        }))
    }

    /// One sweep of every scalar and species on the flux `phi`. `nu_face` (the flow solver's face
    /// viscosity) drives the diffusivity of Schmidt-number entries. Returns the largest residual.
    pub fn iterate(&mut self, phi: &[f64], nu_face: Option<&[f64]>) -> f64 {
        let mut worst: f64 = 0.0;
        for e in self.passive.iter_mut().chain(self.species.iter_mut()) {
            if e.schmidt > 0.0 {
                if let Some(nf) = nu_face {
                    e.transport.diffusivity_face = Some(nf.iter().map(|v| v / e.schmidt).collect());
                }
            }
            e.res = e.transport.iterate(phi);
            worst = worst.max(e.res);
        }
        worst
    }

    /// Residual of the last sweep per entry.
    pub fn residuals(&self) -> Vec<(String, f64)> {
        self.passive
            .iter()
            .chain(&self.species)
            .map(|e| (e.name.clone(), e.res))
            .collect()
    }

    /// Cell values of a scalar, species or the balance species.
    pub fn field(&self, name: &str) -> Option<Vec<f64>> {
        if let Some(e) = self.passive.iter().chain(&self.species).find(|e| e.name == name) {
            return Some(e.transport.field.values.clone());
        }
        if !self.balance.is_empty() && name == self.balance {
            let n = self.species.first().map_or(0, |e| e.transport.field.values.len());
            let mut y = vec![1.0; n];
            for e in &self.species {
                for (a, b) in y.iter_mut().zip(&e.transport.field.values) {
                    *a -= b;
                }
            }
            return Some(y);
        }
        None
    }

    /// Names of every available field (scalars, transported species and the balance species).
    pub fn field_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .passive
            .iter()
            .chain(&self.species)
            .map(|e| e.name.clone())
            .collect();
        if !self.balance.is_empty() {
            v.push(self.balance.clone());
        }
        v
    }

    /// Net outflow of a scalar or species through `patch` (all boundaries minus the integrated source
    /// when `patch` is empty). The balance species follows from the closure.
    pub fn flux(&self, name: &str, patch: &str, phi: &[f64]) -> Result<f64> {
        let one = |e: &Entry| -> Result<f64> {
            let fl: Vec<PatchFlux> = e.transport.boundary_flux(phi);
            if patch.is_empty() {
                return Ok(e.transport.imbalance(phi));
            }
            fl.iter()
                .find(|f| f.patch == patch)
                .map(PatchFlux::total)
                .ok_or_else(|| Error::PathNotFound(format!("patch '{patch}'")))
        };
        if let Some(e) = self.passive.iter().chain(&self.species).find(|e| e.name == name) {
            return one(e);
        }
        if name == self.balance && !self.balance.is_empty() {
            // volume outflow minus the other species' outflow
            let geom = &self.species[0].transport.geom;
            let vol: f64 = if patch.is_empty() {
                (geom.n_internal..geom.mesh.n_faces())
                    .filter(|&f| !geom.empty[f])
                    .map(|f| phi[f])
                    .sum()
            } else {
                let p = geom
                    .mesh
                    .patch(patch)
                    .ok_or_else(|| Error::PathNotFound(format!("patch '{patch}'")))?;
                p.faces().filter(|&f| !geom.empty[f]).map(|f| phi[f]).sum()
            };
            let mut others = 0.0;
            for e in &self.species {
                others += one(e)?;
            }
            return Ok(vol - others);
        }
        Err(Error::PathNotFound(format!("scalar or species '{name}'")))
    }

    /// Values of a scalar on its boundary faces (one per boundary face).
    pub fn boundary_values(&self, name: &str) -> Option<Vec<f64>> {
        let e = self.passive.iter().chain(&self.species).find(|e| e.name == name)?;
        Some(e.transport.field.boundary_values(&e.transport.geom))
    }

    /// Cell values as vectors for checkpoints.
    pub fn named_values(&self) -> Vec<(String, Vec<f64>)> {
        self.passive
            .iter()
            .chain(&self.species)
            .map(|e| (e.name.clone(), e.transport.field.values.clone()))
            .collect()
    }

    /// Clip every transported scalar and species to `[lo, hi]`; returns the number of cells changed.
    pub fn clip(&mut self, lo: f64, hi: f64) -> usize {
        let mut n = 0;
        for e in self.passive.iter_mut().chain(self.species.iter_mut()) {
            for v in e.transport.field.values.iter_mut() {
                if *v < lo || *v > hi {
                    *v = v.clamp(lo, hi);
                    n += 1;
                }
            }
        }
        n
    }

    /// Restore values (checkpoint); unknown names are ignored.
    pub fn restore(&mut self, name: &str, values: &[f64]) {
        if let Some(e) = self
            .passive
            .iter_mut()
            .chain(self.species.iter_mut())
            .find(|e| e.name == name)
        {
            if e.transport.field.values.len() == values.len() {
                e.transport.field.values.copy_from_slice(values);
            }
        }
    }
}
