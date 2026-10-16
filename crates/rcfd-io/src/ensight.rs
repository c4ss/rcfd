//! EnSight Gold export: case, geometry and variable files, ASCII and C Binary (F-103, ADR 0048 D6).
//!
//! Written from the public description of the Gold file formats (layouts read from the vendor's public user manual page:
//! case file sections, geometry and per-node / per-element variable files, `nfaced` and `hexa8` element sections,
//! 80-character binary records, wildcard file names). Unverified against any third-party reader: the node order of
//! `hexa8` (same as the common hexahedron convention: bottom face counter-clockwise seen from the top face), the outward
//! orientation of `nfaced` faces and the binary integer / float widths (4 byte, little endian) are from memory.
//!
//! One part, no node or element ids. Cells that are hexahedra with a positive node-order Jacobian become `hexa8`, all
//! others `nfaced`. Per-element variable values are written in element-block order (all `hexa8` cells in cell order, then
//! all `nfaced` cells in cell order). Vector variables are written component-major (all x, then all y, then all z).
//! A time series (`steps.len() > 1`) uses one time set with `*` wildcard variable file names; the geometry is static.

use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use std::path::{Path, PathBuf};

/// Output encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Text, `%10d` integers and `%12.5e` reals.
    Ascii,
    /// C Binary: 80-character records, 32-bit little-endian integers and 32-bit floats.
    Binary,
}

/// Variable data.
#[derive(Debug, Clone, Copy)]
pub enum Data<'a> {
    /// One value per cell.
    CellScalar(&'a [f64]),
    /// One vector per cell.
    CellVector(&'a [Vec3]),
    /// One value per mesh point.
    NodeScalar(&'a [f64]),
    /// One vector per mesh point.
    NodeVector(&'a [Vec3]),
}

/// A named variable.
#[derive(Debug, Clone, Copy)]
pub struct Variable<'a> {
    /// Name (letters, digits and underscores).
    pub name: &'a str,
    /// Values.
    pub data: Data<'a>,
}

/// One time level.
#[derive(Debug, Clone)]
pub struct Step<'a> {
    /// Time value.
    pub time: f64,
    /// Variables (the same names and kinds at every step).
    pub variables: Vec<Variable<'a>>,
}

/// Generated files: `(file name, bytes)`.
#[derive(Debug, Clone)]
pub struct Files {
    /// The files, case file first.
    pub files: Vec<(String, Vec<u8>)>,
}

fn bad(msg: String) -> Error {
    Error::Io(format!("ensight: {msg}"))
}

fn safe_name(what: &str, s: &str) -> Result<()> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(bad(format!(
            "{what} '{s}' must be non-empty and use only letters, digits and underscores"
        )));
    }
    Ok(())
}

/// C `%12.5e` (sign, one digit, five decimals, `e`, sign and at least two exponent digits, right-justified).
pub fn fmt_e12(x: f64) -> String {
    let s = format!("{x:.5e}");
    let (m, e) = s.split_once('e').expect("exponent");
    let ev: i32 = e.parse().expect("integer exponent");
    let out = format!("{m}e{}{:02}", if ev < 0 { '-' } else { '+' }, ev.abs());
    format!("{out:>12}")
}

/// Element layout of the mesh in EnSight terms.
struct Layout {
    /// Cells written as `hexa8` with their 0-based point ids (cell order).
    hexa: Vec<(usize, [usize; 8])>,
    /// Other cells (cell order).
    poly: Vec<usize>,
}

fn outward_loop(mesh: &Mesh, c: usize, f: usize) -> Vec<usize> {
    let l = mesh.face_points(f);
    if f >= mesh.n_internal_faces() || mesh.owner()[f] == c {
        l.to_vec()
    } else {
        l.iter().rev().copied().collect()
    }
}

fn hex_nodes(mesh: &Mesh, c: usize) -> Option<[usize; 8]> {
    let faces = mesh.cell_faces(c);
    if faces.len() != 6 {
        return None;
    }
    let loops: Vec<Vec<usize>> = faces.iter().map(|&f| outward_loop(mesh, c, f)).collect();
    if loops.iter().any(|l| l.len() != 4) {
        return None;
    }
    let mut all: Vec<usize> = loops.iter().flatten().copied().collect();
    all.sort_unstable();
    all.dedup();
    if all.len() != 8 {
        return None;
    }
    // bottom: first face reversed (its right-hand normal then points into the cell); top: the face sharing no point
    let bottom: Vec<usize> = loops[0].iter().rev().copied().collect();
    let top_i = (1..6).find(|&i| loops[i].iter().all(|p| !bottom.contains(p)))?;
    let mut up = [usize::MAX; 4];
    for (i, l) in loops.iter().enumerate() {
        if i == 0 || i == top_i {
            continue;
        }
        for k in 0..4 {
            let (u, v) = (l[k], l[(k + 1) % 4]);
            for (j, b) in bottom.iter().enumerate() {
                if *b == u && loops[top_i].contains(&v) {
                    up[j] = v;
                }
                if *b == v && loops[top_i].contains(&u) {
                    up[j] = u;
                }
            }
        }
    }
    if up.contains(&usize::MAX) {
        return None;
    }
    let n = [bottom[0], bottom[1], bottom[2], bottom[3], up[0], up[1], up[2], up[3]];
    let p = mesh.points();
    let jac = vec3::dot(
        vec3::cross(vec3::sub(p[n[1]], p[n[0]]), vec3::sub(p[n[3]], p[n[0]])),
        vec3::sub(p[n[4]], p[n[0]]),
    );
    if jac > 0.0 {
        Some(n)
    } else {
        None
    }
}

fn layout(mesh: &Mesh) -> Layout {
    let mut hexa = Vec::new();
    let mut poly = Vec::new();
    for c in 0..mesh.n_cells() {
        match hex_nodes(mesh, c) {
            Some(n) => hexa.push((c, n)),
            None => poly.push(c),
        }
    }
    Layout { hexa, poly }
}

/// Text or binary record writer.
struct Out {
    enc: Encoding,
    buf: Vec<u8>,
}

impl Out {
    fn line(&mut self, s: &str) {
        match self.enc {
            Encoding::Ascii => {
                self.buf.extend_from_slice(s.as_bytes());
                self.buf.push(b'\n');
            }
            Encoding::Binary => {
                // fixed 80-byte record, zero padded like a C character array
                let mut b = [0u8; 80];
                let n = s.len().min(80);
                b[..n].copy_from_slice(&s.as_bytes()[..n]);
                self.buf.extend_from_slice(&b);
            }
        }
    }
    fn int(&mut self, v: usize) {
        match self.enc {
            Encoding::Ascii => self.buf.extend_from_slice(format!("{v:>10}\n").as_bytes()),
            Encoding::Binary => self.buf.extend_from_slice(&(v as i32).to_le_bytes()),
        }
    }
    /// Integers on one text line (binary: consecutive ints).
    fn ints(&mut self, v: &[usize]) {
        match self.enc {
            Encoding::Ascii => {
                let s: String = v.iter().map(|x| format!("{x:>10}")).collect();
                self.buf.extend_from_slice(s.as_bytes());
                self.buf.push(b'\n');
            }
            Encoding::Binary => {
                for x in v {
                    self.buf.extend_from_slice(&(*x as i32).to_le_bytes());
                }
            }
        }
    }
    fn real(&mut self, x: f64) {
        match self.enc {
            Encoding::Ascii => {
                self.buf.extend_from_slice(fmt_e12(x).as_bytes());
                self.buf.push(b'\n');
            }
            Encoding::Binary => self.buf.extend_from_slice(&(x as f32).to_le_bytes()),
        }
    }
}

fn check_real(what: &str, x: f64, enc: Encoding) -> Result<()> {
    if !x.is_finite() || (enc == Encoding::Binary && !(x as f32).is_finite()) {
        return Err(bad(format!(
            "{what} is not finite (or exceeds the single-precision range)"
        )));
    }
    Ok(())
}

fn geometry(mesh: &Mesh, lay: &Layout, enc: Encoding, stem: &str) -> Result<Vec<u8>> {
    let mut o = Out { enc, buf: Vec::new() };
    if enc == Encoding::Binary {
        o.line("C Binary");
    }
    o.line("rcfd EnSight Gold export");
    o.line(stem);
    o.line("node id off");
    o.line("element id off");
    o.line("part");
    o.int(1);
    o.line(stem);
    o.line("coordinates");
    o.int(mesh.n_points());
    for k in 0..3 {
        for (i, p) in mesh.points().iter().enumerate() {
            check_real(&format!("point {i}"), p[k], enc)?;
            o.real(p[k]);
        }
    }
    if !lay.hexa.is_empty() {
        o.line("hexa8");
        o.int(lay.hexa.len());
        for (_, n) in &lay.hexa {
            let ids: Vec<usize> = n.iter().map(|v| v + 1).collect();
            o.ints(&ids);
        }
    }
    if !lay.poly.is_empty() {
        o.line("nfaced");
        o.int(lay.poly.len());
        let loops: Vec<Vec<Vec<usize>>> = lay
            .poly
            .iter()
            .map(|&c| mesh.cell_faces(c).iter().map(|&f| outward_loop(mesh, c, f)).collect())
            .collect();
        for l in &loops {
            o.int(l.len());
        }
        for l in &loops {
            for f in l {
                o.int(f.len());
            }
        }
        for l in &loops {
            for f in l {
                let ids: Vec<usize> = f.iter().map(|v| v + 1).collect();
                o.ints(&ids);
            }
        }
    }
    Ok(o.buf)
}

fn kind_key(d: &Data) -> (&'static str, bool) {
    match d {
        Data::CellScalar(_) => ("scalar per element", false),
        Data::CellVector(_) => ("vector per element", false),
        Data::NodeScalar(_) => ("scalar per node", true),
        Data::NodeVector(_) => ("vector per node", true),
    }
}

fn variable_file(mesh: &Mesh, lay: &Layout, enc: Encoding, v: &Variable) -> Result<Vec<u8>> {
    let mut o = Out { enc, buf: Vec::new() };
    o.line(v.name);
    o.line("part");
    o.int(1);
    let (expect, per_node) = match v.data {
        Data::CellScalar(_) | Data::CellVector(_) => (mesh.n_cells(), false),
        _ => (mesh.n_points(), true),
    };
    let n = match v.data {
        Data::CellScalar(a) | Data::NodeScalar(a) => a.len(),
        Data::CellVector(a) | Data::NodeVector(a) => a.len(),
    };
    if n != expect {
        return Err(bad(format!(
            "variable '{}' has {n} values, expected {expect} ({})",
            v.name,
            if per_node { "mesh points" } else { "cells" }
        )));
    }
    let comp = |k: usize, idx: &[usize], o: &mut Out| -> Result<()> {
        for &i in idx {
            let x = match v.data {
                Data::CellScalar(a) | Data::NodeScalar(a) => a[i],
                Data::CellVector(a) | Data::NodeVector(a) => a[i][k],
            };
            check_real(&format!("variable '{}' value {i}", v.name), x, enc)?;
            o.real(x);
        }
        Ok(())
    };
    let ncomp = match v.data {
        Data::CellScalar(_) | Data::NodeScalar(_) => 1,
        _ => 3,
    };
    if per_node {
        o.line("coordinates");
        let idx: Vec<usize> = (0..mesh.n_points()).collect();
        for k in 0..ncomp {
            comp(k, &idx, &mut o)?;
        }
    } else {
        let blocks: [(&str, Vec<usize>); 2] = [
            ("hexa8", lay.hexa.iter().map(|(c, _)| *c).collect()),
            ("nfaced", lay.poly.clone()),
        ];
        for (name, idx) in blocks.iter().filter(|(_, i)| !i.is_empty()) {
            o.line(name);
            for k in 0..ncomp {
                comp(k, idx, &mut o)?;
            }
        }
    }
    Ok(o.buf)
}

/// Build all files in memory. `stem` names the files (`<stem>.case`, `<stem>.geo`, `<stem>.<variable>[.NNNN]`).
pub fn build(mesh: &Mesh, stem: &str, enc: Encoding, steps: &[Step]) -> Result<Files> {
    safe_name("stem", stem)?;
    if steps.is_empty() {
        return Err(bad("at least one time step is required".into()));
    }
    let first = &steps[0].variables;
    for (i, s) in steps.iter().enumerate() {
        if !s.time.is_finite() || (i > 0 && s.time <= steps[i - 1].time) {
            return Err(bad(format!("step {i}: times must be finite and strictly increasing")));
        }
        if s.variables.len() != first.len()
            || s.variables
                .iter()
                .zip(first)
                .any(|(a, b)| a.name != b.name || kind_key(&a.data) != kind_key(&b.data))
        {
            return Err(bad(format!(
                "step {i}: variables differ from step 0 (same names and kinds required)"
            )));
        }
    }
    for (i, v) in first.iter().enumerate() {
        safe_name("variable name", v.name)?;
        if first[..i].iter().any(|w| w.name == v.name) {
            return Err(bad(format!("duplicate variable name '{}'", v.name)));
        }
    }
    let lay = layout(mesh);
    let transient = steps.len() > 1;
    let digits = ((steps.len() - 1).to_string().len()).max(4);
    let file_of = |name: &str, i: usize| -> String {
        if transient {
            format!("{stem}.{name}.{i:0digits$}")
        } else {
            format!("{stem}.{name}")
        }
    };
    let mut case = String::new();
    case.push_str("FORMAT\ntype: ensight gold\n\nGEOMETRY\n");
    case.push_str(&format!("model: {stem}.geo\n"));
    if !first.is_empty() {
        case.push_str("\nVARIABLE\n");
        for v in first {
            let (kind, _) = kind_key(&v.data);
            if transient {
                case.push_str(&format!(
                    "{kind}: 1 {} {stem}.{}.{}\n",
                    v.name,
                    v.name,
                    "*".repeat(digits)
                ));
            } else {
                case.push_str(&format!("{kind}: {} {stem}.{}\n", v.name, v.name));
            }
        }
    }
    if transient {
        case.push_str(&format!(
            "\nTIME\ntime set: 1\nnumber of steps: {}\nfilename start number: 0\nfilename increment: 1\ntime values:",
            steps.len()
        ));
        for (i, s) in steps.iter().enumerate() {
            case.push_str(if i % 6 == 0 { "\n" } else { " " });
            case.push_str(&format!("{}", s.time));
        }
        case.push('\n');
    }
    let mut files = vec![(format!("{stem}.case"), case.into_bytes())];
    files.push((format!("{stem}.geo"), geometry(mesh, &lay, enc, stem)?));
    for (i, s) in steps.iter().enumerate() {
        for v in &s.variables {
            files.push((file_of(v.name, i), variable_file(mesh, &lay, enc, v)?));
        }
    }
    Ok(Files { files })
}

/// Write the files into `dir` (created if missing); returns the paths, case file first.
pub fn write(dir: impl AsRef<Path>, stem: &str, mesh: &Mesh, enc: Encoding, steps: &[Step]) -> Result<Vec<PathBuf>> {
    let f = build(mesh, stem, enc, steps)?;
    let dir = dir.as_ref();
    std::fs::create_dir_all(dir)?;
    let mut out = Vec::new();
    for (name, bytes) in f.files {
        let p = dir.join(name);
        std::fs::write(&p, bytes)?;
        out.push(p);
    }
    Ok(out)
}

/// Number of cells written as `hexa8` and as `nfaced`.
pub fn element_counts(mesh: &Mesh) -> (usize, usize) {
    let l = layout(mesh);
    (l.hexa.len(), l.poly.len())
}
