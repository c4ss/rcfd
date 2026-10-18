//! Surface-data input for the impermeable FW-H solver: the `rcfd-surface-series` v1 format (JSON geometry plus a
//! pressure time series per face, inline or as a CSV table), a builder to a moving [`crate::fwh::Surface`], and an
//! adapter from the post-processing surface sampling (ADR 0066).
//!
//! ```json
//! {"format": "rcfd-surface-series", "version": 1, "rho0": 1.2, "c0": 340.0, "t0": 0.0, "dt": 1e-4,
//!  "mode": "impermeable", "flow": [0, 0, 20.0],
//!  "motion": {"type": "rotation", "axis": [0, 0, 1], "origin": [0, 0, 0], "omega": 150.0, "angle0": 0.0,
//!             "velocity": [0, 0, 0]},
//!  "faces": [{"area": 1e-3, "x": [0.2, 0, 0], "n": [0, 0, 1], "p": [..]}],
//!  "pressure_csv": "p.csv"}
//! ```
//!
//! Face positions `x` and unit normals `n` are the reference values at `t0` in the body frame; `p` is the acoustic
//! pressure `p - p_ref` (the mean is NOT removed: steady loading on a rotating blade is a source). With
//! `pressure_csv` instead of `p`, the CSV has the header `t,f0,f1,...` (one column per face in file order, uniform
//! time column). `motion` defaults to static; for a rotation the position is `origin + R(axis, angle0 + omega (t - t0))
//! (x - origin) + velocity (t - t0)`, the normal rotates likewise and `v = omega axis x (x - origin) + velocity`.
//! `flow` is the uniform stream past the body (body frame); the medium-frame transformation is applied by
//! [`SeriesCase::medium_frame`]. `mode` is `"impermeable"` (thickness + loading, default) or `"loading"`.

use crate::fwh::{Face, SourceMode, Surface};
use serde_json::{json, Value};

/// Error of the surface-series reader; messages carry the JSON path or CSV row.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesError(pub String);

impl std::fmt::Display for SeriesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SeriesError {}

type V3 = [f64; 3];
type Res<T> = Result<T, SeriesError>;

fn err<T>(m: String) -> Res<T> {
    Err(SeriesError(m))
}

/// Body motion.
#[derive(Debug, Clone, PartialEq)]
pub enum Motion {
    /// Fixed in the body frame.
    Static,
    /// Rotation about an axis through `origin` with rate `omega` (rad/s), angle `angle0` at `t0`, plus a translation.
    Rotation {
        /// Axis direction (normalised on use).
        axis: V3,
        /// A point on the axis.
        origin: V3,
        /// Angular velocity.
        omega: f64,
        /// Angle at `t0`.
        angle0: f64,
        /// Translation velocity.
        velocity: V3,
    },
}

/// One face as read: reference geometry and pressure series.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceSpec {
    /// Area, m^2.
    pub area: f64,
    /// Reference position at `t0`.
    pub x: V3,
    /// Reference unit normal at `t0`.
    pub n: V3,
    /// Pressure series `p'`.
    pub p: Vec<f64>,
}

/// The parsed document.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesSpec {
    /// Ambient density.
    pub rho0: f64,
    /// Speed of sound.
    pub c0: f64,
    /// First sample time.
    pub t0: f64,
    /// Sample interval.
    pub dt: f64,
    /// Source terms.
    pub mode: SourceMode,
    /// Uniform stream past the body (body frame).
    pub flow: V3,
    /// Body motion.
    pub motion: Motion,
    /// Faces.
    pub faces: Vec<FaceSpec>,
}

/// A built case: the surface in the body frame plus the stream.
#[derive(Debug, Clone, PartialEq)]
pub struct SeriesCase {
    /// Body-frame impermeable surface (positions and velocities relative to the body-fixed axes).
    pub surface: Surface,
    /// Uniform stream past the body.
    pub flow: V3,
}

impl SeriesCase {
    /// The surface in the frame where the medium is at rest (identity for zero flow).
    pub fn medium_frame(&self) -> Result<Surface, crate::fwh::FwhError> {
        if self.flow == [0.0; 3] {
            Ok(self.surface.clone())
        } else {
            self.surface.into_medium_frame(self.flow)
        }
    }
}

fn norm3(a: V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn rotate(axis: V3, angle: f64, p: V3) -> V3 {
    // Rodrigues, axis unit
    let (s, c) = angle.sin_cos();
    let d = axis[0] * p[0] + axis[1] * p[1] + axis[2] * p[2];
    let cr = [
        axis[1] * p[2] - axis[2] * p[1],
        axis[2] * p[0] - axis[0] * p[2],
        axis[0] * p[1] - axis[1] * p[0],
    ];
    [
        p[0] * c + cr[0] * s + axis[0] * d * (1.0 - c),
        p[1] * c + cr[1] * s + axis[1] * d * (1.0 - c),
        p[2] * c + cr[2] * s + axis[2] * d * (1.0 - c),
    ]
}

impl SeriesSpec {
    /// Number of samples.
    pub fn samples(&self) -> usize {
        self.faces.first().map_or(0, |f| f.p.len())
    }

    /// Check values (finite, positive, unit normals, equal series lengths, subsonic motion is checked by the FW-H
    /// validation). Messages carry the JSON path.
    pub fn validate(&self) -> Res<()> {
        for (name, v) in [("rho0", self.rho0), ("c0", self.c0), ("dt", self.dt)] {
            if !(v.is_finite() && v > 0.0) {
                return err(format!("$.{name}: must be finite and positive (got {v})"));
            }
        }
        if !self.t0.is_finite() {
            return err(format!("$.t0: must be finite (got {})", self.t0));
        }
        if self.flow.iter().any(|c| !c.is_finite()) {
            return err("$.flow: components must be finite".into());
        }
        if let Motion::Rotation {
            axis,
            origin,
            omega,
            angle0,
            velocity,
        } = &self.motion
        {
            if !(norm3(*axis).is_finite() && norm3(*axis) > 0.0) {
                return err("$.motion.axis: must be a nonzero finite vector".into());
            }
            if !omega.is_finite() || !angle0.is_finite() {
                return err("$.motion: omega and angle0 must be finite".into());
            }
            if origin.iter().chain(velocity.iter()).any(|c| !c.is_finite()) {
                return err("$.motion: origin and velocity components must be finite".into());
            }
        }
        if self.faces.is_empty() {
            return err("$.faces: at least one face is required".into());
        }
        let ns = self.samples();
        if ns < 8 {
            return err(format!("$.faces[0].p: need at least 8 samples (got {ns})"));
        }
        for (k, f) in self.faces.iter().enumerate() {
            if !(f.area.is_finite() && f.area > 0.0) {
                return err(format!(
                    "$.faces[{k}].area: must be finite and positive (got {})",
                    f.area
                ));
            }
            if f.x.iter().any(|c| !c.is_finite()) {
                return err(format!("$.faces[{k}].x: components must be finite"));
            }
            if f.n.iter().any(|c| !c.is_finite()) || (norm3(f.n) - 1.0).abs() > 1e-6 {
                return err(format!("$.faces[{k}].n: must be a unit vector (norm {})", norm3(f.n)));
            }
            if f.p.len() != ns {
                return err(format!("$.faces[{k}].p: expected {ns} samples, got {}", f.p.len()));
            }
            if let Some(i) = f.p.iter().position(|v| !v.is_finite()) {
                return err(format!("$.faces[{k}].p[{i}]: not finite"));
            }
        }
        Ok(())
    }

    /// Build the body-frame impermeable surface.
    pub fn build(&self) -> Res<SeriesCase> {
        self.validate()?;
        let ns = self.samples();
        let mut faces = Vec::with_capacity(self.faces.len());
        for f in &self.faces {
            let (mut xs, mut ns_, mut vs) = (Vec::with_capacity(ns), Vec::with_capacity(ns), Vec::with_capacity(ns));
            for i in 0..ns {
                let t = i as f64 * self.dt; // t - t0
                match &self.motion {
                    Motion::Static => {
                        xs.push(f.x);
                        ns_.push(f.n);
                        vs.push([0.0; 3]);
                    }
                    Motion::Rotation {
                        axis,
                        origin,
                        omega,
                        angle0,
                        velocity,
                    } => {
                        let nrm = norm3(*axis);
                        let a = [axis[0] / nrm, axis[1] / nrm, axis[2] / nrm];
                        let ang = angle0 + omega * t;
                        let rel = [f.x[0] - origin[0], f.x[1] - origin[1], f.x[2] - origin[2]];
                        let r = rotate(a, ang, rel);
                        xs.push([
                            origin[0] + r[0] + velocity[0] * t,
                            origin[1] + r[1] + velocity[1] * t,
                            origin[2] + r[2] + velocity[2] * t,
                        ]);
                        ns_.push(rotate(a, ang, f.n));
                        vs.push([
                            omega * (a[1] * r[2] - a[2] * r[1]) + velocity[0],
                            omega * (a[2] * r[0] - a[0] * r[2]) + velocity[1],
                            omega * (a[0] * r[1] - a[1] * r[0]) + velocity[2],
                        ]);
                    }
                }
            }
            faces.push(Face {
                area: f.area,
                x: xs,
                n: ns_,
                v: vs,
                p: f.p.clone(),
            });
        }
        Ok(SeriesCase {
            surface: Surface::impermeable(self.rho0, self.c0, self.t0, self.dt, faces, self.mode),
            flow: self.flow,
        })
    }

    /// Serialise (inline pressure series).
    pub fn to_json(&self) -> Value {
        let motion = match &self.motion {
            Motion::Static => json!({"type": "static"}),
            Motion::Rotation {
                axis,
                origin,
                omega,
                angle0,
                velocity,
            } => json!({"type": "rotation", "axis": axis, "origin": origin, "omega": omega, "angle0": angle0,
                         "velocity": velocity}),
        };
        let faces: Vec<Value> = self
            .faces
            .iter()
            .map(|f| json!({"area": f.area, "x": f.x, "n": f.n, "p": f.p}))
            .collect();
        json!({"format": "rcfd-surface-series", "version": 1, "rho0": self.rho0, "c0": self.c0, "t0": self.t0,
               "dt": self.dt,
               "mode": match self.mode { SourceMode::Impermeable => "impermeable", SourceMode::LoadingOnly => "loading" },
               "flow": self.flow, "motion": motion, "faces": faces})
    }

    /// Pressure table as CSV (`t,f0,f1,...`), full precision.
    pub fn pressure_csv(&self) -> String {
        let mut s = String::from("t");
        for k in 0..self.faces.len() {
            s.push_str(&format!(",f{k}"));
        }
        s.push('\n');
        for i in 0..self.samples() {
            s.push_str(&format!("{:e}", self.t0 + i as f64 * self.dt));
            for f in &self.faces {
                s.push_str(&format!(",{:e}", f.p[i]));
            }
            s.push('\n');
        }
        s
    }

    /// Parse the JSON document. `csv` supplies the text of the pressure table when the document names a
    /// `pressure_csv` (the caller resolves the path, see [`pressure_csv_path`]).
    pub fn from_json(doc: &Value, csv: Option<&str>) -> Res<SeriesSpec> {
        let obj = doc
            .as_object()
            .ok_or_else(|| SeriesError("$: expected an object".into()))?;
        match obj.get("format").and_then(Value::as_str) {
            Some("rcfd-surface-series") => {}
            other => return err(format!("$.format: expected \"rcfd-surface-series\", got {other:?}")),
        }
        if obj.get("version").and_then(Value::as_u64) != Some(1) {
            return err("$.version: only version 1 is supported".into());
        }
        let num = |k: &str| -> Res<f64> {
            obj.get(k)
                .and_then(Value::as_f64)
                .ok_or_else(|| SeriesError(format!("$.{k}: missing or not a number")))
        };
        let vec3 = |a: &Value, path: &str| -> Res<V3> {
            let arr = a
                .as_array()
                .filter(|a| a.len() == 3)
                .ok_or_else(|| SeriesError(format!("{path}: expected [x, y, z]")))?;
            let mut r = [0.0; 3];
            for (i, x) in arr.iter().enumerate() {
                r[i] = x
                    .as_f64()
                    .ok_or_else(|| SeriesError(format!("{path}[{i}]: not a number")))?;
            }
            Ok(r)
        };
        let (rho0, c0, t0, dt) = (num("rho0")?, num("c0")?, num("t0")?, num("dt")?);
        let mode = match obj.get("mode").and_then(Value::as_str) {
            None | Some("impermeable") => SourceMode::Impermeable,
            Some("loading") => SourceMode::LoadingOnly,
            Some(o) => return err(format!("$.mode: expected \"impermeable\" or \"loading\", got \"{o}\"")),
        };
        let flow = match obj.get("flow") {
            Some(v) => vec3(v, "$.flow")?,
            None => [0.0; 3],
        };
        let motion = match obj.get("motion") {
            None => Motion::Static,
            Some(m) => match m.get("type").and_then(Value::as_str) {
                Some("static") => Motion::Static,
                Some("rotation") => {
                    let g = |k: &str| m.get(k).ok_or_else(|| SeriesError(format!("$.motion.{k}: missing")));
                    Motion::Rotation {
                        axis: vec3(g("axis")?, "$.motion.axis")?,
                        origin: match m.get("origin") {
                            Some(o) => vec3(o, "$.motion.origin")?,
                            None => [0.0; 3],
                        },
                        omega: g("omega")?
                            .as_f64()
                            .ok_or_else(|| SeriesError("$.motion.omega: not a number".into()))?,
                        angle0: match m.get("angle0") {
                            Some(a) => a
                                .as_f64()
                                .ok_or_else(|| SeriesError("$.motion.angle0: not a number".into()))?,
                            None => 0.0,
                        },
                        velocity: match m.get("velocity") {
                            Some(v) => vec3(v, "$.motion.velocity")?,
                            None => [0.0; 3],
                        },
                    }
                }
                other => {
                    return err(format!(
                        "$.motion.type: expected \"static\" or \"rotation\", got {other:?}"
                    ))
                }
            },
        };
        let els = obj
            .get("faces")
            .and_then(Value::as_array)
            .ok_or_else(|| SeriesError("$.faces: missing or not an array".into()))?;
        let table = match (obj.contains_key("pressure_csv"), csv) {
            (true, Some(text)) => Some(parse_pressure_csv(text)?),
            (true, None) => return err("$.pressure_csv: named but no CSV text was supplied".into()),
            _ => None,
        };
        if let Some((tt0, tdt, cols)) = &table {
            if (tt0 - t0).abs() > 1e-9 * dt.abs().max(1e-300) || (tdt - dt).abs() > 1e-9 * dt.abs() {
                return err(format!(
                    "$.pressure_csv: time axis (t0 = {tt0}, dt = {tdt}) differs from the document (t0 = {t0}, dt = {dt})"
                ));
            }
            if cols.len() != els.len() {
                return err(format!(
                    "$.pressure_csv: {} face columns for {} faces",
                    cols.len(),
                    els.len()
                ));
            }
        }
        let mut faces = Vec::new();
        for (k, e) in els.iter().enumerate() {
            let at = |f: &str| format!("$.faces[{k}].{f}");
            let area = e
                .get("area")
                .and_then(Value::as_f64)
                .ok_or_else(|| SeriesError(format!("{}: missing or not a number", at("area"))))?;
            let x = vec3(
                e.get("x").ok_or_else(|| SeriesError(format!("{}: missing", at("x"))))?,
                &at("x"),
            )?;
            let n = vec3(
                e.get("n").ok_or_else(|| SeriesError(format!("{}: missing", at("n"))))?,
                &at("n"),
            )?;
            let p = match (&table, e.get("p")) {
                (Some((_, _, cols)), _) => cols[k].clone(),
                (None, Some(v)) => {
                    let arr = v
                        .as_array()
                        .ok_or_else(|| SeriesError(format!("{}: expected an array", at("p"))))?;
                    arr.iter()
                        .enumerate()
                        .map(|(i, x)| {
                            x.as_f64()
                                .ok_or_else(|| SeriesError(format!("{}[{i}]: not a number", at("p"))))
                        })
                        .collect::<Res<Vec<f64>>>()?
                }
                (None, None) => return err(format!("{}: missing (and no pressure_csv)", at("p"))),
            };
            faces.push(FaceSpec { area, x, n, p });
        }
        let s = SeriesSpec {
            rho0,
            c0,
            t0,
            dt,
            mode,
            flow,
            motion,
            faces,
        };
        s.validate()?;
        Ok(s)
    }
}

/// The `pressure_csv` path named by a document, if any.
pub fn pressure_csv_path(doc: &Value) -> Option<&str> {
    doc.get("pressure_csv").and_then(Value::as_str)
}

/// Parse a pressure table: header `t,f0,f1,...`, rows of numbers, uniform time column. Returns `(t0, dt, columns)`.
pub fn parse_pressure_csv(text: &str) -> Res<(f64, f64, Vec<Vec<f64>>)> {
    let mut lines = text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty());
    let (_, header) = lines
        .next()
        .ok_or_else(|| SeriesError("pressure_csv: empty file".into()))?;
    let ncol = header.split(',').count();
    if ncol < 2 {
        return err("pressure_csv row 1: header needs a time column and at least one face column".into());
    }
    let mut t = Vec::new();
    let mut cols: Vec<Vec<f64>> = vec![Vec::new(); ncol - 1];
    for (ln, line) in lines {
        let cells: Vec<&str> = line.split(',').collect();
        if cells.len() != ncol {
            return err(format!(
                "pressure_csv row {}: expected {ncol} columns, got {}",
                ln + 1,
                cells.len()
            ));
        }
        let mut vals = Vec::with_capacity(ncol);
        for (c, s) in cells.iter().enumerate() {
            let v: f64 = s.trim().parse().map_err(|_| {
                SeriesError(format!(
                    "pressure_csv row {}, column {}: not a number ('{s}')",
                    ln + 1,
                    c + 1
                ))
            })?;
            if !v.is_finite() {
                return err(format!("pressure_csv row {}, column {}: not finite", ln + 1, c + 1));
            }
            vals.push(v);
        }
        t.push(vals[0]);
        for (c, v) in vals[1..].iter().enumerate() {
            cols[c].push(*v);
        }
    }
    if t.len() < 2 {
        return err("pressure_csv: need at least two rows".into());
    }
    let dt = t[1] - t[0];
    if dt.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return err("pressure_csv row 3: time column must increase".into());
    }
    for i in 1..t.len() {
        if (t[i] - t[i - 1] - dt).abs() > 1e-6 * dt {
            return err(format!(
                "pressure_csv row {}: time step {} differs from the uniform step {dt} of the first interval",
                i + 2,
                t[i] - t[i - 1]
            ));
        }
    }
    Ok((t[0], dt, cols))
}

/// Geometry of the facets of a post-processing [`crate::surfaces::Surface`]: `(area, centroid, unit normal)` per
/// triangle. `flip` reverses the normal (the cut normal points to increasing level-set value; FW-H needs the normal
/// pointing out of the body into the fluid). Degenerate triangles are an error naming the index.
pub fn faces_from_post_surface(surf: &crate::surfaces::Surface, flip: bool) -> Res<Vec<(f64, V3, V3)>> {
    let av = surf.area_vectors();
    let cen = surf.centroids();
    let mut out = Vec::with_capacity(av.len());
    for (i, (a, c)) in av.iter().zip(&cen).enumerate() {
        let area = norm3(*a);
        if !(area.is_finite() && area > 0.0) {
            return err(format!("tris[{i}]: degenerate facet (area {area})"));
        }
        let s = if flip { -1.0 / area } else { 1.0 / area };
        out.push((area, *c, [a[0] * s, a[1] * s, a[2] * s]));
    }
    Ok(out)
}

/// Combine adapter geometry and per-facet pressure series (`p[face][sample]`) into face specs.
pub fn face_specs(geom: &[(f64, V3, V3)], p: &[Vec<f64>]) -> Res<Vec<FaceSpec>> {
    if geom.len() != p.len() {
        return err(format!("p: {} series for {} facets", p.len(), geom.len()));
    }
    Ok(geom
        .iter()
        .zip(p)
        .map(|(g, p)| FaceSpec {
            area: g.0,
            x: g.1,
            n: g.2,
            p: p.clone(),
        })
        .collect())
}
