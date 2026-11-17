//! Derived pictures of a stored field (ADR 0738 P4, F-809): iso-surfaces, streamlines, slices and the Studio's cut with
//! its arrows, computed from a run's field store (`rcfd_tsstore`) by `rcfd_post` and written as a GLB. One function for
//! the MCP project tool `derive_picture` and the command `rcfd post derive` (G16), so both give the same file.
//!
//! The arguments are a JSON object (the tool's, or the command's flags made into one):
//!
//! - `kind`: `iso` (`field`, `level`, optional `colour_by`), `streamlines` (`seeds`, optional `direction`, `max_steps`,
//!   `max_length`, `min_speed`, `step_fraction`), `slice` (`field`, `axis`, `at`, optional `vectors`) or `cut_surface`
//!   (optional `field`, `axis` with `at` and `keep`, `vectors`);
//! - the snapshot: `snapshot` (index), `step` or `time` (the nearest), default the last.
//!
//! Fields are the stored cell fields and `speed`, `|(ux, uy, uz)|` of the stored components (`uz` taken as 0 when the
//! store has none, as a 2D case's). Errors: 1202 no such field or snapshot; 1003 a point field, a missing velocity
//! component, a level outside the field's range; 1004 over the limits ([`MAX_TRIANGLES`], [`MAX_SEEDS`],
//! [`MAX_STEPS`]); -32602 a bad argument.

use crate::{err, RpcError, NOT_FOUND, PARAMS, REFUSED, VALIDATION};
use rcfd_mesh::Mesh;
use rcfd_post::picture::{
    add_triangles, clip_segments, clip_surface, mesh_layers, slice, store_surface, vector_components, vector_glyphs,
    Cut, GlbBuilder, Triangles,
};
use rcfd_post::streamlines::{patch_seeds, rake, trace, Directions, TraceOptions};
use rcfd_tsstore::{Location, Pick, Reader};
use serde_json::{json, Map, Value};

/// The kinds of picture.
pub const KINDS: [&str; 4] = ["iso", "streamlines", "slice", "cut_surface"];
/// Most triangles a picture may have (ADR 0738 Q7 (a)).
pub const MAX_TRIANGLES: usize = 2_000_000;
/// Most seeds of a streamline picture.
pub const MAX_SEEDS: usize = 1000;
/// Largest `max_steps`.
pub const MAX_STEPS: usize = 10_000;
/// The velocity components.
const U: [&str; 3] = ["ux", "uy", "uz"];
/// Axis names.
const AXES: [&str; 3] = ["x", "y", "z"];

/// A derived picture: the GLB and what the reply says of it.
#[derive(Debug, Clone)]
pub struct Derived {
    /// The file.
    pub glb: Vec<u8>,
    /// `{kind, snapshot, triangles | lines | points, bounds, length_unit, range, stats, mesh_hash}`.
    pub summary: Value,
}

fn params(m: impl Into<String>) -> RpcError {
    err(PARAMS, m)
}

fn num(args: &Value, k: &str) -> Result<Option<f64>, RpcError> {
    match args.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite())
            .map(Some)
            .ok_or_else(|| params(format!("'{k}' must be a finite number"))),
    }
}

fn uint(args: &Value, k: &str) -> Result<Option<u64>, RpcError> {
    match args.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .map(Some)
            .ok_or_else(|| params(format!("'{k}' must be a non-negative integer"))),
    }
}

fn text<'a>(args: &'a Value, k: &str) -> Result<Option<&'a str>, RpcError> {
    match args.get(k) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| params(format!("'{k}' must be a string"))),
    }
}

fn point(v: &Value, what: &str) -> Result<[f64; 3], RpcError> {
    let a = v.as_array().filter(|a| a.len() == 3);
    let p: Option<Vec<f64>> = a.map(|a| a.iter().filter_map(Value::as_f64).filter(|x| x.is_finite()).collect());
    match p {
        Some(p) if p.len() == 3 => Ok([p[0], p[1], p[2]]),
        _ => Err(params(format!("{what} must be [x, y, z] of finite numbers"))),
    }
}

/// The snapshot index the arguments ask for (`snapshot`, `step` or `time`, nearest; default the last).
pub fn snapshot_index(r: &Reader, args: &Value) -> Result<usize, RpcError> {
    let n = r.snapshots().len();
    if n == 0 {
        return Err(err(NOT_FOUND, "the store holds no snapshot yet"));
    }
    let given = ["snapshot", "step", "time"]
        .iter()
        .filter(|k| args.get(**k).is_some_and(|v| !v.is_null()))
        .count();
    if given > 1 {
        return Err(params("give one of snapshot, step and time"));
    }
    if let Some(i) = uint(args, "snapshot")? {
        if i as usize >= n {
            return Err(err(
                NOT_FOUND,
                format!("no snapshot {i}: the store holds {n} (0 to {})", n - 1),
            ));
        }
        return Ok(i as usize);
    }
    if let Some(s) = uint(args, "step")? {
        return r
            .by_step(s, Pick::Nearest)
            .ok_or_else(|| err(NOT_FOUND, format!("no snapshot for step {s}")));
    }
    if let Some(t) = num(args, "time")? {
        return r
            .by_time(t, Pick::Nearest)
            .ok_or_else(|| err(NOT_FOUND, format!("no snapshot for time {t}")));
    }
    Ok(n - 1)
}

struct Store<'a> {
    r: &'a mut Reader,
    i: usize,
    n_cells: usize,
}

impl Store<'_> {
    fn ts(e: impl std::fmt::Display) -> RpcError {
        err(crate::INTERNAL, format!("field store: {e}"))
    }

    fn names(&mut self) -> Result<Vec<(String, Location, usize)>, RpcError> {
        Ok(self
            .r
            .fields(self.i)
            .map_err(Self::ts)?
            .into_iter()
            .map(|f| (f.name, f.location, f.n))
            .collect())
    }

    /// A stored cell field, `None` when the snapshot has no field of that name.
    fn stored(&mut self, name: &str) -> Result<Option<Vec<f64>>, RpcError> {
        let Some((_, loc, n)) = self.names()?.into_iter().find(|f| f.0 == name) else {
            return Ok(None);
        };
        if loc != Location::Cell || n != self.n_cells {
            return Err(err(
                VALIDATION,
                format!("'{name}' is a point field; derived pictures use cell fields"),
            ));
        }
        Ok(Some(self.r.read_field(self.i, name).map_err(Self::ts)?))
    }

    /// The velocity components (`uz` zero when not stored); ux and uy must be stored.
    fn velocity(&mut self) -> Result<[Vec<f64>; 3], RpcError> {
        let mut out: [Vec<f64>; 3] = Default::default();
        for (k, name) in U.iter().enumerate() {
            out[k] = match self.stored(name)? {
                Some(v) => v,
                None if k == 2 => vec![0.0; self.n_cells],
                None => {
                    return Err(err(
                        VALIDATION,
                        format!("the store holds no cell field {name}: velocity needs ux and uy (and uz on a 3D case)"),
                    ))
                }
            };
        }
        Ok(out)
    }

    /// A field of the pictures: a stored cell field, or `speed`.
    fn field(&mut self, name: &str) -> Result<Vec<f64>, RpcError> {
        if let Some(v) = self.stored(name)? {
            return Ok(v);
        }
        if name == "speed" {
            let u = self.velocity()?;
            return Ok((0..self.n_cells)
                .map(|c| (u[0][c] * u[0][c] + u[1][c] * u[1][c] + u[2][c] * u[2][c]).sqrt())
                .collect());
        }
        let names: Vec<String> = self.names()?.into_iter().map(|f| f.0).collect();
        Err(RpcError {
            code: NOT_FOUND,
            message: format!(
                "snapshot {} has no field '{name}' (the fields are the stored cell fields and speed)",
                self.i
            ),
            data: Some(json!({"fields": names})),
        })
    }
}

fn axis_arg(args: &Value) -> Result<Option<usize>, RpcError> {
    match text(args, "axis")? {
        None => Ok(None),
        Some("x") => Ok(Some(0)),
        Some("y") => Ok(Some(1)),
        Some("z") => Ok(Some(2)),
        Some(o) => Err(params(format!("axis '{o}' is not x, y or z"))),
    }
}

fn range(v: impl Iterator<Item = f32>) -> Value {
    let (lo, hi) = v.fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), x| (a.min(x), b.max(x)));
    if lo <= hi {
        json!([lo, hi])
    } else {
        Value::Null
    }
}

fn bounds(points: impl Iterator<Item = [f32; 3]>) -> Value {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for p in points {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if lo[0] <= hi[0] {
        json!({"min": lo, "max": hi})
    } else {
        Value::Null
    }
}

fn too_many(n: usize, what: &str) -> Result<(), RpcError> {
    if n > MAX_TRIANGLES {
        return Err(RpcError {
            code: REFUSED,
            message: format!("the {what} has {n} triangles, over the limit of {MAX_TRIANGLES}"),
            data: Some(json!({"triangles": n, "limit": MAX_TRIANGLES})),
        });
    }
    Ok(())
}

/// The picture the arguments ask for, of snapshot `i` of the store (see the module comment).
pub fn derive(r: &mut Reader, i: usize, args: &Value) -> Result<Derived, RpcError> {
    if !args.is_object() {
        return Err(params("the arguments must be an object"));
    }
    let kind = text(args, "kind")?.ok_or_else(|| params("missing 'kind' (iso, streamlines, slice or cut_surface)"))?;
    if !KINDS.contains(&kind) {
        return Err(params(format!(
            "kind '{kind}' is not one of iso, streamlines, slice, cut_surface"
        )));
    }
    let mesh = r.mesh().map_err(Store::ts)?;
    let n_cells = mesh.n_cells();
    let snap = &r.snapshots()[i];
    let snapshot = json!({"index": i, "step": snap.step, "time": snap.time});
    let mesh_hash = format!("{:016x}", r.mesh_hash());
    let mut st = Store { r, i, n_cells };
    let (b, mut summary) = match kind {
        "iso" => iso(&mesh, &mut st, args)?,
        "streamlines" => streamlines(&mesh, &mut st, args)?,
        "slice" => slice_kind(&mesh, &mut st, args)?,
        _ => cut_surface(&mesh, &mut st, args)?,
    };
    summary["kind"] = json!(kind);
    summary["snapshot"] = snapshot;
    summary["mesh_hash"] = json!(mesh_hash);
    summary["length_unit"] = json!(rcfd_tsstore::LENGTH_UNIT);
    let glb = finish(b, kind, &summary)?;
    Ok(Derived { glb, summary })
}

fn finish(b: GlbBuilder, kind: &str, summary: &Value) -> Result<Vec<u8>, RpcError> {
    b.finish(&format!("rcfd-{kind}"), summary.clone())
        .map_err(|e| err(crate::INTERNAL, e.to_string()))
}

fn iso(mesh: &Mesh, st: &mut Store, args: &Value) -> Result<(GlbBuilder, Value), RpcError> {
    let name = text(args, "field")?.ok_or_else(|| params("iso needs 'field'"))?;
    let level = num(args, "level")?.ok_or_else(|| params("iso needs 'level'"))?;
    let v = st.field(name)?;
    let (lo, hi) = v
        .iter()
        .filter(|x| x.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    if !(level > lo && level < hi) {
        return Err(RpcError {
            code: VALIDATION,
            message: format!("level {level} is not inside the range of '{name}', {lo} to {hi}"),
            data: Some(json!({"range": [lo, hi]})),
        });
    }
    let colour = match text(args, "colour_by")? {
        Some(c) => Some((c.to_string(), st.field(c)?)),
        None => None,
    };
    let s = rcfd_post::iso::iso_surface(mesh, &v, level, colour.as_ref().map(|c| c.1.as_slice()))
        .map_err(|e| err(VALIDATION, e.to_string()))?;
    let surf = &s.surface;
    too_many(surf.tris.len(), "iso-surface")?;
    let pos: Vec<f32> = surf.points.iter().flat_map(|p| p.map(|x| x as f32)).collect();
    let cells: Vec<u32> = surf.vertex_cell.iter().map(|c| c.unwrap_or(0) as u32).collect();
    let vals: Vec<f32> = s.colour.iter().map(|&x| x as f32).collect();
    let idx: Vec<u32> = surf.tris.iter().flatten().map(|&x| x as u32).collect();
    let summary = json!({
        "field": name, "level": level, "colour_by": colour.as_ref().map(|c| c.0.clone()),
        "triangles": surf.tris.len(), "vertices": surf.points.len(),
        "bounds": bounds(pos.chunks_exact(3).map(|p| [p[0], p[1], p[2]])), "range": range(vals.iter().copied()),
        "stats": {"area": rcfd_post::iso::area(surf), "field_range": [lo, hi]},
    });
    let mut b = GlbBuilder::new();
    if !surf.tris.is_empty() {
        let p = b.floats(&pos, 3, true);
        let c = b.uints(&cells);
        let v = b.floats(&vals, 1, false);
        let i = b.indices(&idx);
        b.primitive(&[("POSITION", p), ("_CELL", c), ("_VALUE", v)], Some(i), 4, Value::Null);
    }
    Ok((b, summary))
}

fn seeds(mesh: &Mesh, args: &Value) -> Result<Vec<[f64; 3]>, RpcError> {
    let s = args.get("seeds").ok_or_else(|| params("streamlines need 'seeds'"))?;
    let n_of = |v: &Value| -> Result<usize, RpcError> {
        v.get("n")
            .and_then(Value::as_u64)
            .filter(|&n| n >= 1)
            .map(|n| n as usize)
            .ok_or_else(|| params("seeds: 'n' must be a positive integer"))
    };
    let n_given = |v: &Value| -> Result<(), RpcError> {
        let n = n_of(v)?;
        if n > MAX_SEEDS {
            return Err(RpcError {
                code: REFUSED,
                message: format!("{n} seeds, over the limit of {MAX_SEEDS}"),
                data: Some(json!({"seeds": n, "limit": MAX_SEEDS})),
            });
        }
        Ok(())
    };
    if let Some(rk) = s.get("rake") {
        n_given(rk)?;
        let p0 = point(rk.get("p0").unwrap_or(&Value::Null), "seeds.rake.p0")?;
        let p1 = point(rk.get("p1").unwrap_or(&Value::Null), "seeds.rake.p1")?;
        return Ok(rake(p0, p1, n_of(rk)?));
    }
    if let Some(ps) = s.get("points") {
        let a = ps
            .as_array()
            .ok_or_else(|| params("seeds.points must be an array of [x, y, z]"))?;
        return a.iter().map(|p| point(p, "a seed point")).collect();
    }
    if let Some(name) = s.get("patch") {
        let name = name
            .as_str()
            .ok_or_else(|| params("seeds.patch must be a patch name"))?;
        n_given(s)?;
        return patch_seeds(mesh, name, n_of(s)?).map_err(|e| err(NOT_FOUND, e.to_string()));
    }
    Err(params(
        "seeds must be {rake: {p0, p1, n}}, {points: [[x, y, z], ...]} or {patch: name, n}",
    ))
}

fn streamlines(mesh: &Mesh, st: &mut Store, args: &Value) -> Result<(GlbBuilder, Value), RpcError> {
    let seeds = seeds(mesh, args)?;
    if seeds.len() > MAX_SEEDS {
        return Err(RpcError {
            code: REFUSED,
            message: format!("{} seeds, over the limit of {MAX_SEEDS}", seeds.len()),
            data: Some(json!({"seeds": seeds.len(), "limit": MAX_SEEDS})),
        });
    }
    let direction = match text(args, "direction")?.unwrap_or("forward") {
        "forward" => Directions::Forward,
        "backward" => Directions::Backward,
        "both" => Directions::Both,
        d => return Err(params(format!("direction '{d}' is not forward, backward or both"))),
    };
    let mut o = TraceOptions {
        direction,
        ..Default::default()
    };
    if let Some(m) = uint(args, "max_steps")? {
        if m as usize > MAX_STEPS {
            return Err(RpcError {
                code: REFUSED,
                message: format!("max_steps {m} is over the limit of {MAX_STEPS}"),
                data: Some(json!({"max_steps": m, "limit": MAX_STEPS})),
            });
        }
        o.max_steps = m as usize;
    }
    o.max_length = num(args, "max_length")?;
    o.min_speed = num(args, "min_speed")?;
    if let Some(f) = num(args, "step_fraction")? {
        if !(f > 0.0 && f <= 1.0) {
            return Err(params(format!("step_fraction {f} must be in (0, 1]")));
        }
        o.step_fraction = f;
    }
    let u = st.velocity()?;
    let lines = trace(mesh, [&u[0], &u[1], &u[2]], &seeds, &o).map_err(|e| err(VALIDATION, e.to_string()))?;
    let mut b = GlbBuilder::new();
    let mut stops: Map<String, Value> = Map::new();
    let (mut points, mut drawn) = (0usize, 0usize);
    let mut all: Vec<[f32; 3]> = Vec::new();
    let mut speeds: Vec<f32> = Vec::new();
    for l in &lines {
        *stops.entry(l.stop.name()).or_insert(json!(0)) =
            json!(stops.get(l.stop.name()).and_then(Value::as_u64).unwrap_or(0) + 1);
        points += l.points.len();
        if l.points.len() < 2 {
            continue;
        }
        drawn += 1;
        let pos: Vec<f32> = l.points.iter().flat_map(|p| p.map(|x| x as f32)).collect();
        all.extend(l.points.iter().map(|p| p.map(|x| x as f32)));
        let sp: Vec<f32> = l.speed.iter().map(|&x| x as f32).collect();
        speeds.extend_from_slice(&sp);
        let cells: Vec<u32> = l.cells.iter().map(|&c| c as u32).collect();
        let p = b.floats(&pos, 3, true);
        let c = b.uints(&cells);
        let v = b.floats(&sp, 1, false);
        b.primitive(
            &[("POSITION", p), ("_CELL", c), ("_VALUE", v)],
            None,
            3,
            json!({"seed": l.seed, "direction": l.direction.name(), "stop": l.stop.name(), "length": l.length()}),
        );
    }
    let summary = json!({
        "seeds": seeds.len(), "lines": lines.len(), "lines_drawn": drawn, "points": points,
        "bounds": bounds(all.into_iter()), "range": range(speeds.into_iter()),
        "stats": {"stops": stops, "max_steps": o.max_steps, "step_fraction": o.step_fraction},
    });
    Ok((b, summary))
}

fn cell_values(t: &Triangles, v: &[f64]) -> Vec<f32> {
    t.cells.iter().map(|&c| v[c as usize] as f32).collect()
}

fn slice_kind(mesh: &Mesh, st: &mut Store, args: &Value) -> Result<(GlbBuilder, Value), RpcError> {
    let name = text(args, "field")?.ok_or_else(|| params("slice needs 'field'"))?;
    let axis = axis_arg(args)?.ok_or_else(|| params("slice needs 'axis' (x, y or z)"))?;
    let at = num(args, "at")?.ok_or_else(|| params("slice needs 'at', the plane's coordinate (m)"))?;
    let v = st.field(name)?;
    let t = slice(mesh, axis, at).map_err(|e| err(VALIDATION, e.to_string()))?;
    too_many(t.len(), "slice")?;
    let vals = cell_values(&t, &v);
    let mut b = GlbBuilder::new();
    add_triangles(&mut b, &t, Some(&vals), Value::Null);
    let mut vectors = 0;
    if args.get("vectors").and_then(Value::as_bool).unwrap_or(false) && !t.is_empty() {
        let u = st.velocity()?;
        let centres: Vec<f32> = t
            .corners
            .chunks_exact(3)
            .flat_map(|q| [0, 1, 2].map(|k| ((q[0][k] as f64 + q[1][k] as f64 + q[2][k] as f64) / 3.0) as f32))
            .collect();
        let vecs: Vec<f32> = t
            .cells
            .iter()
            .flat_map(|&c| [0, 1, 2].map(|k| u[k][c as usize] as f32))
            .collect();
        let p = b.floats(&centres, 3, true);
        let c = b.uints(&t.cells);
        let w = b.floats(&vecs, 3, false);
        b.primitive(
            &[("POSITION", p), ("_CELL", c), ("_VECTOR", w)],
            None,
            0,
            json!({"vectors": true}),
        );
        vectors = t.len();
    }
    let summary = json!({
        "field": name, "axis": AXES[axis], "at": at, "triangles": t.len(), "points": vectors,
        "bounds": bounds(t.corners.iter().copied()), "range": range(vals.iter().copied()),
        "stats": {"cells_cut": t.cells.iter().collect::<std::collections::BTreeSet<_>>().len()},
    });
    Ok((b, summary))
}

fn cut_surface(mesh: &Mesh, st: &mut Store, args: &Value) -> Result<(GlbBuilder, Value), RpcError> {
    let s = store_surface(mesh).map_err(|e| err(REFUSED, e.to_string()))?;
    let layers = mesh_layers(&s, st.n_cells);
    let cut = match (axis_arg(args)?, num(args, "at")?) {
        (None, None) => None,
        (Some(axis), Some(at)) => {
            let below = match text(args, "keep")?.unwrap_or("below") {
                "below" => true,
                "above" => false,
                k => return Err(params(format!("keep '{k}' is not below or above"))),
            };
            Some(Cut {
                axis,
                at: at as f32,
                below,
            })
        }
        _ => return Err(params("a cut needs both 'axis' and 'at'")),
    };
    let t = clip_surface(&s, cut.as_ref());
    too_many(t.len(), "cut surface")?;
    let values = match text(args, "field")? {
        Some(name) => Some((name.to_string(), st.field(name)?)),
        None => None,
    };
    let vals = values.as_ref().map(|(_, v)| cell_values(&t, v));
    let mut b = GlbBuilder::new();
    add_triangles(&mut b, &t, vals.as_deref(), json!({"surface": true}));
    let mut arrows = Value::Null;
    if args.get("vectors").and_then(Value::as_bool).unwrap_or(false) {
        let need = vector_components(&layers);
        let mut comps: [Option<Vec<f32>>; 3] = Default::default();
        for &k in &need {
            let v = st.stored(U[k])?.ok_or_else(|| {
                err(
                    VALIDATION,
                    format!(
                        "the store holds no cell field {}; {} need {}",
                        U[k],
                        if layers.two_d {
                            "in-plane arrows of a 2D case"
                        } else {
                            "arrows of a 3D case"
                        },
                        need.iter().map(|&k| U[k]).collect::<Vec<_>>().join(", ")
                    ),
                )
            })?;
            comps[k] = Some(v.iter().map(|&x| x as f32).collect());
        }
        let g = vector_glyphs(
            &s,
            [comps[0].as_deref(), comps[1].as_deref(), comps[2].as_deref()],
            &layers,
            st.n_cells,
        );
        let seg = clip_segments(&g, cut.as_ref());
        if !seg.points.is_empty() {
            let pos: Vec<f32> = seg.points.iter().flatten().copied().collect();
            let cells: Vec<u32> = seg.arrow.iter().flat_map(|&a| [g.cells[a as usize]; 2]).collect();
            let tris: Vec<u32> = seg.arrow.iter().flat_map(|&a| [g.triangles[a as usize]; 2]).collect();
            let p = b.floats(&pos, 3, true);
            let c = b.uints(&cells);
            let tr = b.uints(&tris);
            b.primitive(
                &[("POSITION", p), ("_CELL", c), ("_TRIANGLE", tr)],
                None,
                1,
                json!({"arrows": g.cells.len()}),
            );
        }
        arrows = json!({"arrows": g.cells.len(), "segments": seg.points.len(), "vmax": g.vmax, "step": g.step});
    }
    let summary = json!({
        "field": values.as_ref().map(|v| v.0.clone()), "triangles": t.len(), "surface_triangles": s.n_triangles(),
        "cut": cut.map(|c| json!({"axis": AXES[c.axis], "at": c.at, "keep": if c.below { "below" } else { "above" }})),
        "bounds": bounds(t.corners.iter().copied()), "range": vals.as_ref().map_or(Value::Null, |v| range(v.iter().copied())),
        "stats": {"two_d": layers.two_d, "thin": AXES[layers.thin], "min": layers.min, "max": layers.max,
            "vectors": arrows},
    });
    Ok((b, summary))
}

/// The file name of a picture in a run's `derived/` folder: `<kind>-<fnv64 of the arguments, snapshot and mesh>.glb`
/// (the arguments without `run`, keys sorted).
pub fn file_name(kind: &str, args: &Value, snapshot: usize, mesh_hash: u64) -> String {
    let mut a = args.clone();
    if let Some(o) = a.as_object_mut() {
        for k in ["run", "snapshot", "step", "time"] {
            o.remove(k);
        }
    }
    let key = format!(
        "{}|{snapshot}|{mesh_hash:016x}",
        serde_json::to_string(&a).unwrap_or_default()
    );
    let h = rcfd_project::records::hash_bytes(key.as_bytes());
    format!("{kind}-{}.glb", h.trim_start_matches("fnv64:"))
}

impl crate::Workspace {
    /// The project tool `derive_picture` (ADR 0738 P4): the picture of a stored snapshot of run `run`, written to
    /// `runs/<id>/derived/<kind>-<hash>.glb` with its reply beside it (`.json`); an existing file of that name is reused.
    pub(crate) fn derive_picture(&self, args: &Value) -> Result<Value, RpcError> {
        use rcfd_tsstore::STORE_FILE;
        let (id, rundir) = self.run_arg(args, "run")?;
        let store = rundir.join("fields");
        if !store.join(STORE_FILE).is_file() {
            return Err(err(
                NOT_FOUND,
                format!("run {id} has no stored fields (runs/{id}/fields/{STORE_FILE})"),
            ));
        }
        let mut r = Reader::open(&store).map_err(|e| err(crate::INTERNAL, format!("runs/{id}/fields: {e}")))?;
        let i = snapshot_index(&r, args)?;
        let kind = text(args, "kind")?.unwrap_or("");
        if !KINDS.contains(&kind) {
            return Err(params(format!(
                "kind '{kind}' is not one of iso, streamlines, slice, cut_surface"
            )));
        }
        let name = file_name(kind, args, i, r.mesh_hash());
        let rel = format!("runs/{id}/derived/{name}");
        let path = self.project().root().join(&rel);
        let side = path.with_extension("json");
        let with_path = |mut v: Value, reused: bool| {
            v["run"] = json!(id);
            v["path"] = json!(rel);
            v["reused"] = json!(reused);
            v
        };
        if path.is_file() {
            if let Ok(v) = rcfd_project::atomic::read_json::<Value>(&side) {
                return Ok(with_path(v, true));
            }
        }
        let mut a = args.clone();
        if let Some(o) = a.as_object_mut() {
            o.remove("run");
        }
        let d = derive(&mut r, i, &a)?;
        std::fs::create_dir_all(path.parent().expect("has a parent"))
            .map_err(|e| err(crate::INTERNAL, format!("{rel}: {e}")))?;
        rcfd_project::atomic::atomic_write(&path, &d.glb).map_err(crate::project_error)?;
        let mut v = d.summary;
        v["bytes"] = json!(d.glb.len());
        rcfd_project::atomic::atomic_write(&side, v.to_string().as_bytes()).map_err(crate::project_error)?;
        Ok(with_path(v, false))
    }
}

/// The picture of the field store in `dir` (`runs/<id>/fields`), the snapshot the arguments ask for (`rcfd post derive`).
pub fn derive_dir(dir: &std::path::Path, args: &Value) -> Result<Derived, RpcError> {
    let mut r = Reader::open(dir).map_err(|e| err(NOT_FOUND, format!("{}: {e}", dir.display())))?;
    let i = snapshot_index(&r, args)?;
    derive(&mut r, i, args)
}
