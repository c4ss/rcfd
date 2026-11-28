//! `mesh.preview` (F-836, ADR 0732): the mesh a session's case runs on, built without solving and written as a
//! mesh-only field store (no snapshot, no field) the Studio reads with its store reader.
//!
//! * The mesh is the one `Simulation::new` sets the run up on: `sim::build_mesh` for a plain case (what
//!   `Simulation::new` calls), and `Simulation::new`'s own mesh for a case with `[mesh.adapt]` (the base lattice) or
//!   `[mesh.sliding]` (the joined blocks). With `adapt: "final"` a `[mesh.adapt]` case runs the run's adaptive loop
//!   (`sim::adapted_mesh`, `cycles + 1` solves) and the store holds its final mesh (ADR 0732 Q3 (a)).
//! * Before a generator mesh is built its cells are estimated from the `mesh.n*` counts; over [`ESTIMATE_LIMIT`] the
//!   call is refused (1004) unless `force: true` (Q4 (b)). After any build, more cells than the service's `max_cells`
//!   are refused (1004) before anything is written.
//! * The store goes to `<dir>/<mesh hash>/` (Q2 (a)): written in a temporary folder beside it and renamed. A folder of
//!   that name that already holds a store of the same mesh hash is used again without writing (`reused: true`).
//!
//! Opt-in: only a service started with `rcfd serve --ui` has the method (Q1 (a), `crate::ui::UI_METHODS`).

use crate::case::{Case, Diagnostic};
use crate::service::{code, Rpc, RpcError};
use crate::sim::{self, Simulation};
use rcfd_mesh::{Mesh, PatchKind};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The cell estimate above which a generator mesh is not built without `force: true` (the egui preview's limit, ADR
/// 0138).
pub const ESTIMATE_LIMIT: u64 = 2_000_000;

/// Parameters `mesh.preview` takes.
pub const PARAMS: [&str; 4] = ["session", "dir", "adapt", "force"];

/// Which mesh of a `[mesh.adapt]` case to preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adapt {
    /// The mesh the run starts on (its first step changes it).
    Base,
    /// The mesh the run solves on after its first step (runs the adaptive loop).
    Final,
}

/// The checked parameters (everything but the session).
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// The folder of previews; the store goes to `<dir>/<mesh hash>/`.
    pub dir: String,
    /// Base or final mesh.
    pub adapt: Adapt,
    /// Build even when the estimate is over [`ESTIMATE_LIMIT`].
    pub force: bool,
}

impl Request {
    /// Check the parameters: an object of [`PARAMS`] only, `session` and `dir` strings, `adapt` `"base"` or `"final"`,
    /// `force` a boolean. -32602 otherwise.
    pub fn from_json(p: &Value) -> Rpc<Request> {
        let o = p
            .as_object()
            .ok_or_else(|| RpcError::params("mesh.preview takes an object {session, dir, adapt?, force?}"))?;
        if let Some(k) = o.keys().find(|k| !PARAMS.contains(&k.as_str())) {
            return Err(RpcError::params(format!(
                "unknown parameter '{k}' (mesh.preview takes {})",
                PARAMS.join(", ")
            )));
        }
        if !o.get("session").is_some_and(Value::is_string) {
            return Err(RpcError::params("missing string parameter 'session'"));
        }
        let dir = match o.get("dir") {
            Some(Value::String(d)) if !d.is_empty() => d.clone(),
            _ => return Err(RpcError::params("missing string parameter 'dir'")),
        };
        let adapt = match o.get("adapt") {
            None | Some(Value::Null) => Adapt::Base,
            Some(Value::String(s)) if s == "base" => Adapt::Base,
            Some(Value::String(s)) if s == "final" => Adapt::Final,
            Some(v) => {
                return Err(RpcError::params(format!(
                    "'adapt' must be \"base\" or \"final\", not {v}"
                )))
            }
        };
        let force = match o.get("force") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(v) => return Err(RpcError::params(format!("'force' must be a boolean, not {v}"))),
        };
        Ok(Request { dir, adapt, force })
    }
}

fn int(case: &Case, path: &str) -> u64 {
    case.get(path).ok().and_then(|v| v.as_i64()).unwrap_or(0).max(0) as u64
}

fn text(case: &Case, path: &str) -> String {
    case.get(path)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// The cells of the case's generator mesh before it is built: the product of its `mesh.n*` counts for the generators
/// whose count is that product (the rule of the egui preview, ADR 0138; checked against the built mesh by a test). None
/// for a mesh file and for a generator counted otherwise.
pub fn estimate(case: &Case) -> Option<u64> {
    if text(case, "mesh.source") == "file" {
        return None;
    }
    let (nx, ny, nz) = (int(case, "mesh.nx"), int(case, "mesh.ny"), int(case, "mesh.nz").max(1));
    let right = |key: &str, same: u64| match int(case, key) {
        0 => same,
        n => n,
    };
    let (nxr, nyr, nzr) = (
        right("mesh.nx_right", nx),
        right("mesh.ny_right", ny),
        right("mesh.nz_right", nz),
    );
    let m = |a: u64, b: u64, c: u64| a.saturating_mul(b).saturating_mul(c);
    Some(match text(case, "mesh.generator").as_str() {
        "cavity_2d" | "box_2d" | "channel_2d" | "wedge_2d" | "nozzle_2d" => m(nx, ny, 1),
        "channel_2d_interface" => m(nx, ny, 1).saturating_add(m(nxr, nyr, 1)),
        "annulus_3d" | "annulus_3d_sector" => m(nx, ny, nz),
        "annulus_3d_interface" => m(nx, ny, nz).saturating_add(m(nx, nyr, nzr)),
        "annulus_3d_stage" | "annulus_3d_cylindrical_interface" => m(nx, ny, nz).saturating_add(m(nxr, nyr, nzr)),
        _ => return None,
    })
}

/// The cells of the case's mesh before it is built (F-989, ADR 0844): for a mesh file the count its reader would build,
/// read from the file as a stream without building it; for a generator [`estimate`] where that gives one, else the
/// count of `cylinder_ogrid`, `pipe_sweep`, `box_3d`, `annulus_3d_sector_cylindrical_interface` and `axial_fan_sector`
/// from their parameters. None where the count is not sure (a file the reader would refuse, an unknown generator): the
/// caller then builds. `case.facts` refuses by it; `mesh.preview` keeps [`estimate`].
pub fn cell_estimate(case: &Case) -> Option<u64> {
    if text(case, "mesh.source") == "file" {
        let file = text(case, "mesh.file");
        let path = if Path::new(&file).is_absolute() {
            PathBuf::from(&file)
        } else {
            case.base_dir.join(&file)
        };
        return file_cells(&path);
    }
    estimate(case).or_else(|| other_generator_cells(case))
}

/// The generators [`estimate`] does not name (ADR 0844, from the code of each generator).
fn other_generator_cells(case: &Case) -> Option<u64> {
    let (nx, ny, nz) = (int(case, "mesh.nx"), int(case, "mesh.ny"), int(case, "mesh.nz"));
    let m = |a: u64, b: u64, c: u64| a.saturating_mul(b).saturating_mul(c);
    let right = |key: &str, same: u64| match int(case, key) {
        0 => same,
        n => n,
    };
    Some(match text(case, "mesh.generator").as_str() {
        "cylinder_ogrid" => m(nx, ny, 1),
        "pipe_sweep" => m(nx, nx, 1).saturating_add(m(4, nx, ny)).saturating_mul(nz),
        "box_3d" => match text(case, crate::dynmesh::SECTION).as_str() {
            "rectangles" => m(nx, ny.max(1), nz.max(1)),
            "triangles" => m(nx, ny.max(1), nz.max(1)).saturating_mul(2),
            _ => m(nx, ny, nz.max(1)),
        },
        "annulus_3d_sector_cylindrical_interface" => {
            let nz = nz.max(1);
            m(nx, ny, nz).saturating_add(m(
                right("mesh.nx_right", nx),
                right("mesh.ny_right", ny),
                right("mesh.nz_right", nz),
            ))
        }
        "axial_fan_sector" => fan_cells(case),
        _ => return None,
    })
}

/// `axial_fan_sector`: the H-grid of `rcfd_mesh::fan_mesh::build_sector_mesh` or the C-grid of `fan_cgrid`.
fn fan_cells(case: &Case) -> u64 {
    let f = |k: &str| int(case, &format!("mesh.fan.{k}"));
    let blade = case
        .get("mesh.fan.include_blade")
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let m = |a: u64, b: u64| a.saturating_mul(b);
    let n_k = f("n_span").saturating_add(f("n_gap"));
    if text(case, "mesh.fan.topology") == "c_grid" {
        let c = m(2 * f("n_down").saturating_add(f("n_chord")), f("n_side"));
        let fe = m(2 * f("n_nose"), f("n_up"));
        let k0 = if blade { f("n_span") } else { 0 };
        let core = m(m(f("n_core"), f("n_chord")), n_k.saturating_sub(k0));
        m(c.saturating_add(fe), n_k).saturating_add(core)
    } else {
        let n_i = f("n_up").saturating_add(f("n_chord")).saturating_add(f("n_down"));
        let n_j = (2 * f("n_side")).saturating_add(f("n_mid"));
        let all = m(m(n_i, n_j), n_k);
        let removed = if blade {
            m(m(f("n_chord"), f("n_mid")), f("n_span"))
        } else {
            0
        };
        all.saturating_sub(removed)
    }
}

/// A file read forward through a buffer, never whole.
struct Stream {
    file: std::fs::File,
    buf: Vec<u8>,
    pos: usize,
    len: usize,
}

impl Stream {
    fn open(path: &Path) -> Option<Stream> {
        Some(Stream {
            file: std::fs::File::open(path).ok()?,
            buf: vec![0; 1 << 20],
            pos: 0,
            len: 0,
        })
    }
    fn fill(&mut self) -> bool {
        if self.pos < self.len {
            return true;
        }
        self.pos = 0;
        self.len = std::io::Read::read(&mut self.file, &mut self.buf).unwrap_or(0);
        self.len > 0
    }
    fn peek(&mut self) -> Option<u8> {
        self.fill().then(|| self.buf[self.pos])
    }
    fn byte(&mut self) -> Option<u8> {
        let b = self.peek()?;
        self.pos += 1;
        Some(b)
    }
    fn skip_ws(&mut self) {
        while self.fill() {
            match self.buf[self.pos..self.len]
                .iter()
                .position(|b| !b.is_ascii_whitespace())
            {
                Some(i) => {
                    self.pos += i;
                    return;
                }
                None => self.pos = self.len,
            }
        }
    }
    /// The next run of non-blank bytes (empty at the end).
    fn token(&mut self, out: &mut Vec<u8>) {
        out.clear();
        self.skip_ws();
        while self.fill() {
            let s = &self.buf[self.pos..self.len];
            match s.iter().position(|b| b.is_ascii_whitespace()) {
                Some(i) => {
                    out.extend_from_slice(&s[..i]);
                    self.pos += i;
                    return;
                }
                None => {
                    out.extend_from_slice(s);
                    self.pos = self.len;
                }
            }
        }
    }
    /// Skip `n` blank-separated tokens.
    fn skip_tokens(&mut self, n: u64) -> Option<()> {
        let mut t = Vec::new();
        for _ in 0..n {
            self.token(&mut t);
            if t.is_empty() {
                return None;
            }
        }
        Some(())
    }
    fn skip_bytes(&mut self, mut n: u64) -> Option<()> {
        let here = ((self.len - self.pos) as u64).min(n);
        self.pos += here as usize;
        n -= here;
        if n > 0 {
            std::io::Seek::seek(&mut self.file, std::io::SeekFrom::Current(n as i64)).ok()?;
            let meta = self.file.metadata().ok()?.len();
            if std::io::Seek::stream_position(&mut self.file).ok()? > meta {
                return None;
            }
        }
        Some(())
    }
    fn bytes<const N: usize>(&mut self, big_endian: bool) -> Option<[u8; N]> {
        let mut a = [0u8; N];
        for x in a.iter_mut() {
            *x = self.byte()?;
        }
        if big_endian {
            a.reverse();
        }
        Some(a)
    }
    /// Read through the first occurrence of `pat` (whose first byte does not recur in it); false if it is not there.
    fn find(&mut self, pat: &[u8]) -> bool {
        let mut k = 0;
        while self.fill() {
            if k == 0 {
                match self.buf[self.pos..self.len].iter().position(|&b| b == pat[0]) {
                    Some(i) => self.pos += i + 1,
                    None => {
                        self.pos = self.len;
                        continue;
                    }
                }
                k = 1;
            } else {
                let b = self.buf[self.pos];
                self.pos += 1;
                k = if b == pat[k] { k + 1 } else { usize::from(b == pat[0]) };
            }
            if k == pat.len() {
                return true;
            }
        }
        false
    }
}

fn parse<T: std::str::FromStr>(t: &[u8]) -> Option<T> {
    std::str::from_utf8(t).ok()?.parse().ok()
}

/// The cells a mesh file's reader would build (`sim::build_mesh`'s file branch): an OpenFOAM folder, a Fluent file
/// (first non-blank byte `(`), else a Gmsh file. None for gzip or HDF5 input, which the readers refuse.
fn file_cells(path: &Path) -> Option<u64> {
    if path.is_dir() {
        return foam_cells(path);
    }
    let mut s = Stream::open(path)?;
    s.fill();
    let head = &s.buf[..s.len];
    if head.starts_with(&[0x1f, 0x8b]) || head.starts_with(b"\x89HDF") {
        return None;
    }
    s.skip_ws();
    if s.peek()? == b'(' {
        fluent_cells(&mut s)
    } else {
        gmsh_cells(&mut s)
    }
}

/// Nodes per Gmsh element type, as `rcfd_io::gmsh` knows them.
fn gmsh_nodes(t: i64) -> Option<u64> {
    const N: [u64; 19] = [2, 3, 4, 4, 8, 6, 5, 3, 6, 9, 10, 27, 18, 14, 1, 8, 20, 15, 13];
    usize::try_from(t)
        .ok()
        .filter(|t| (1..=19).contains(t))
        .map(|t| N[t - 1])
}

/// The element types `rcfd_io::gmsh::classify` makes cells.
fn gmsh_cell(t: i64) -> bool {
    matches!(t, 4 | 11 | 5 | 12 | 17 | 6 | 13 | 18 | 7 | 14 | 19)
}

/// Gmsh 2.x or 4.1, ASCII or binary: the elements of a cell type in `$Elements`.
fn gmsh_cells(s: &mut Stream) -> Option<u64> {
    let mut t = Vec::new();
    let (mut version, mut binary, mut big) = (0.0f64, false, false);
    let mut cells = 0u64;
    loop {
        s.token(&mut t);
        if t.is_empty() {
            break;
        }
        match t.as_slice() {
            b"$MeshFormat" => {
                s.token(&mut t);
                version = parse(&t)?;
                s.token(&mut t);
                binary = parse::<i64>(&t)? == 1;
                s.token(&mut t);
                if binary {
                    newline(s);
                    let one = s.bytes::<4>(false)?;
                    big = match (i32::from_le_bytes(one), i32::from_be_bytes(one)) {
                        (1, _) => false,
                        (_, 1) => true,
                        _ => return None,
                    };
                }
                if !s.find(b"$EndMeshFormat") {
                    return None;
                }
                if !(2.0..3.0).contains(&version) && !(4.05..4.5).contains(&version) {
                    return None;
                }
            }
            b"$Elements" => {
                cells = cells.saturating_add(gmsh_elements(s, version, binary, big)?);
                if !s.find(b"$EndElements") {
                    return None;
                }
            }
            x if x.starts_with(b"$End") => {}
            x if x.starts_with(b"$") => {
                let mut end = b"$End".to_vec();
                end.extend_from_slice(&x[1..]);
                if !s.find(&end) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    (version != 0.0).then_some(cells)
}

fn newline(s: &mut Stream) {
    if s.peek() == Some(b'\r') {
        s.byte();
    }
    if s.peek() == Some(b'\n') {
        s.byte();
    }
}

fn gmsh_elements(s: &mut Stream, version: f64, binary: bool, big: bool) -> Option<u64> {
    let mut t = Vec::new();
    let int = |s: &mut Stream, t: &mut Vec<u8>| -> Option<i64> {
        if binary {
            Some(i64::from(i32::from_le_bytes(s.bytes::<4>(big)?)))
        } else {
            s.token(t);
            parse(t)
        }
    };
    let size = |s: &mut Stream, t: &mut Vec<u8>| -> Option<u64> {
        if binary {
            Some(u64::from_le_bytes(s.bytes::<8>(big)?))
        } else {
            s.token(t);
            parse(t)
        }
    };
    let mut cells = 0u64;
    if version >= 4.0 {
        if binary {
            newline(s);
        }
        let blocks = size(s, &mut t)?;
        for _ in 0..3 {
            size(s, &mut t)?;
        }
        for _ in 0..blocks {
            let _dim = int(s, &mut t)?;
            let _entity = int(s, &mut t)?;
            let ty = int(s, &mut t)?;
            let n = size(s, &mut t)?;
            let values = n.checked_mul(1 + gmsh_nodes(ty)?)?;
            if gmsh_cell(ty) {
                cells = cells.saturating_add(n);
            }
            if binary {
                s.skip_bytes(values.checked_mul(8)?)?;
            } else {
                s.skip_tokens(values)?;
            }
        }
        return Some(cells);
    }
    s.token(&mut t);
    let n: u64 = parse(&t)?;
    if binary {
        newline(s);
        let mut done = 0u64;
        while done < n {
            let ty = int(s, &mut t)?;
            let num = u64::try_from(int(s, &mut t)?).ok()?;
            let tags = u64::try_from(int(s, &mut t)?).ok()?;
            let per = 1 + tags + gmsh_nodes(ty)?;
            if gmsh_cell(ty) {
                cells = cells.saturating_add(num);
            }
            s.skip_bytes(num.checked_mul(per)?.checked_mul(4)?)?;
            done = done.checked_add(num)?;
        }
    } else {
        for _ in 0..n {
            s.skip_tokens(1)?;
            let ty = int(s, &mut t)?;
            let tags = u64::try_from(int(s, &mut t)?).ok()?;
            if gmsh_cell(ty) {
                cells += 1;
            }
            s.skip_tokens(tags + gmsh_nodes(ty)?)?;
        }
    }
    Some(cells)
}

/// Read a parenthesis group whose `(` is already consumed (string-aware, as `rcfd_io::fluent` balances it); with
/// `keep`, its bytes up to the closing `)`.
fn fluent_group(s: &mut Stream, keep: Option<&mut Vec<u8>>) -> Option<()> {
    let mut depth = 1usize;
    let mut keep = keep;
    while depth > 0 {
        if keep.is_none() {
            // fast path: jump to the next byte that matters
            s.fill().then_some(())?;
            let i = s.buf[s.pos..s.len]
                .iter()
                .position(|&b| b == b'(' || b == b')' || b == b'"');
            match i {
                Some(i) => s.pos += i,
                None => {
                    s.pos = s.len;
                    continue;
                }
            }
        }
        let b = s.byte()?;
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'"' => loop {
                let c = s.byte()?;
                if let Some(k) = keep.as_mut() {
                    k.push(c);
                }
                if c == b'"' {
                    break;
                }
            },
            _ => {}
        }
        if depth > 0 {
            if let Some(k) = keep.as_mut() {
                k.push(b);
            }
        }
    }
    Some(())
}

/// The cells of a cell-section header `zone first last type ...` (hexadecimal): `last` for a zone that is not 0 and a
/// type that is not 0 (the reader's cell zones).
fn fluent_header_cells(h: &[u8]) -> Option<Option<u64>> {
    let h = std::str::from_utf8(h).ok()?;
    let hx: Vec<&str> = h.split_whitespace().collect();
    let num = |i: usize| u64::from_str_radix(hx.get(i)?, 16).ok();
    let zone = num(0)?;
    let last = num(2)?;
    let ty = match hx.get(3) {
        Some(_) => num(3)?,
        None => 1,
    };
    Some((zone != 0 && ty != 0).then_some(last))
}

/// Fluent: the largest `last` of the cell sections 12, 2012 and 3012, the sections walked as `split_sections` walks them.
fn fluent_cells(s: &mut Stream) -> Option<u64> {
    let mut cells: Option<u64> = None;
    let mut digits = Vec::new();
    loop {
        s.skip_ws();
        match s.peek() {
            None => break,
            Some(b'(') => {
                s.byte();
            }
            Some(_) => return None,
        }
        s.skip_ws();
        digits.clear();
        while let Some(b) = s.peek().filter(u8::is_ascii_digit) {
            digits.push(b);
            s.byte();
        }
        let index: Option<u64> = parse(&digits);
        let mut header = Vec::new();
        match index {
            Some(i) if i >= 2000 => {
                s.skip_ws();
                if s.byte()? != b'(' {
                    return None;
                }
                fluent_group(s, Some(&mut header))?;
                s.skip_ws();
                match s.byte()? {
                    b')' => {}
                    b'(' => {
                        if !s.find(b"End of Binary Section") {
                            return None;
                        }
                        while s.byte()? != b')' {}
                        s.skip_ws();
                        if s.peek() == Some(b')') {
                            s.byte();
                        }
                    }
                    _ => return None,
                }
                if i == 2012 || i == 3012 {
                    if let Some(l) = fluent_header_cells(&header)? {
                        cells = Some(cells.map_or(l, |c| c.max(l)));
                    }
                }
            }
            Some(12) => {
                s.skip_ws();
                if s.byte()? != b'(' {
                    return None;
                }
                fluent_group(s, Some(&mut header))?;
                if let Some(l) = fluent_header_cells(&header)? {
                    cells = Some(cells.map_or(l, |c| c.max(l)));
                }
                fluent_group(s, None)?;
            }
            _ => fluent_group(s, None)?,
        }
    }
    cells
}

/// OpenFOAM: the largest label of `owner` and `neighbour` plus one; None for binary data.
fn foam_cells(dir: &Path) -> Option<u64> {
    let mut max: Option<u64> = None;
    for name in ["owner", "neighbour"] {
        let mut s = Stream::open(&dir.join(name))?;
        if !s.find(b"FoamFile") {
            return None;
        }
        s.skip_ws();
        if s.byte()? != b'{' {
            return None;
        }
        let mut header = Vec::new();
        loop {
            let b = s.byte()?;
            if b == b'}' {
                break;
            }
            header.push(b);
        }
        let h = String::from_utf8_lossy(&header);
        let words: Vec<&str> = h
            .split(|c: char| c.is_whitespace() || c == ';')
            .filter(|w| !w.is_empty())
            .collect();
        if words.windows(2).any(|w| w[0] == "format" && w[1] == "binary") {
            return None;
        }
        // comments between the header and the list, then the count and the list
        let mut t = Vec::new();
        loop {
            s.skip_ws();
            if s.peek()? == b'/' {
                while s.byte()? != b'\n' {}
                continue;
            }
            s.token(&mut t);
            break;
        }
        let n: u64 = parse(&t)?;
        s.skip_ws();
        if s.byte()? != b'(' {
            return None;
        }
        let mut v = 0u64;
        let mut digits = 0u64;
        let mut seen = 0u64;
        loop {
            let b = s.byte()?;
            if b.is_ascii_digit() {
                v = v.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
                digits += 1;
                continue;
            }
            if digits > 0 {
                max = Some(max.map_or(v, |m| m.max(v)));
                seen += 1;
                (v, digits) = (0, 0);
            }
            if b == b')' {
                break;
            }
            if !b.is_ascii_whitespace() {
                return None;
            }
        }
        if seen != n {
            return None;
        }
    }
    max.map(|m| m + 1)
}

/// The mesh `Simulation::new(case)` sets the run up on: `build_mesh` for a plain case (`Simulation::new` calls it),
/// `Simulation::new`'s own mesh for `[mesh.adapt]` and `[mesh.sliding]`, whose run meshes differ from `build_mesh`'s.
pub fn run_mesh(case: &Case) -> Result<Mesh, Vec<Diagnostic>> {
    if sim::has_mesh_adapt(case) || sim::has_mesh_sliding(case) {
        return Simulation::new(case).map(|s| (*s.geom.mesh).clone());
    }
    sim::build_mesh(case)
}

/// What `adapt` reports for a `[mesh.adapt]` case: `pending` when the run's first step changes the mesh shown.
fn adapt_json(case: &Case, pending: bool, solves: usize) -> Value {
    json!({"pending": pending, "cycles": int(case, "mesh.adapt.cycles"), "solves": solves})
}

fn bounds(mesh: &Mesh) -> Value {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in mesh.points() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if mesh.n_points() == 0 {
        lo = [0.0; 3];
        hi = [0.0; 3];
    }
    json!({"min": lo, "max": hi})
}

/// The hexadecimal mesh hash the reply and the folder name use (16 digits, as the Studio's store reader prints it).
pub fn hash_hex(mesh: &Mesh) -> String {
    format!("{:016x}", mesh.content_hash())
}

static TMP: AtomicU64 = AtomicU64::new(0);

fn internal(message: String) -> RpcError {
    RpcError::new(code::INTERNAL, message)
}

/// True when `dir` holds a store of `mesh` (same hash, cells and points).
fn holds(dir: &Path, mesh: &Mesh) -> bool {
    rcfd_tsstore::Reader::open(dir).is_ok_and(|r| {
        r.mesh_hash() == mesh.content_hash() && r.n_cells() == mesh.n_cells() && r.n_points() == mesh.n_points()
    })
}

/// Write `mesh` as a mesh-only store at `<dir>/<hash>/` (temporary folder, then rename); `Ok(true)` when a store of
/// that mesh was there already. On failure nothing is left: neither the temporary folder nor `<dir>/<hash>/`.
fn write_store(dir: &Path, mesh: &Mesh) -> Rpc<(PathBuf, bool)> {
    let hash = hash_hex(mesh);
    let path = dir.join(&hash);
    if holds(&path, mesh) {
        return Ok((path, true));
    }
    std::fs::create_dir_all(dir).map_err(|e| internal(format!("cannot create {}: {e}", dir.display())))?;
    let tmp = dir.join(format!(
        ".tmp-{hash}-{}-{}",
        std::process::id(),
        TMP.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| -> Result<(), String> {
        let w = rcfd_tsstore::Writer::create(&tmp, mesh, rcfd_tsstore::WriteOptions::default())
            .map_err(|e| e.to_string())?;
        w.finish().map_err(|e| e.to_string())?;
        if path.exists() {
            // a folder of this name that does not hold this mesh (damaged, or not a store): replaced
            std::fs::remove_dir_all(&path).map_err(|e| format!("cannot replace {}: {e}", path.display()))?;
        }
        std::fs::rename(&tmp, &path).map_err(|e| format!("cannot rename to {}: {e}", path.display()))
    })();
    match written {
        Ok(()) => Ok((path, false)),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            // another call may have written the same mesh in the meantime
            if holds(&path, mesh) {
                return Ok((path, true));
            }
            Err(internal(format!("the preview store cannot be written: {e}")))
        }
    }
}

/// The work of `mesh.preview` once the session's case, its paths and `dir` are checked (`dir` resolved in the sandbox).
pub fn preview(case: &Case, dir: &Path, req: &Request, max_cells: usize) -> Rpc {
    if !req.force {
        if let Some(n) = estimate(case) {
            if n > ESTIMATE_LIMIT {
                return Err(RpcError::new(
                    code::LIMIT,
                    format!(
                        "about {n} cells (from the mesh.n* counts of generator \"{}\"), more than the preview limit of \
                         {ESTIMATE_LIMIT}; pass force: true to build it anyway",
                        text(case, "mesh.generator")
                    ),
                ));
            }
        }
    }
    let adaptive = sim::has_mesh_adapt(case);
    let (mesh, adapt) = if adaptive && req.adapt == Adapt::Final {
        let a = sim::adapted_mesh(case, &|| false)
            .map_err(|e| {
                RpcError::validation(vec![Diagnostic {
                    path: "mesh.adapt".into(),
                    line: None,
                    message: format!("the adaptive loop failed: {e}"),
                }])
            })?
            .ok_or_else(|| internal("the adaptive loop stopped".into()))?;
        let solves = a.cycles.len();
        ((*a.mesh).clone(), adapt_json(case, false, solves))
    } else {
        let mesh = run_mesh(case).map_err(RpcError::validation)?;
        let adapt = if adaptive {
            adapt_json(case, true, 0)
        } else {
            Value::Null
        };
        (mesh, adapt)
    };
    if mesh.n_cells() > max_cells {
        return Err(RpcError::new(
            code::LIMIT,
            format!("mesh has {} cells, limit is {max_cells}", mesh.n_cells()),
        ));
    }
    let (path, reused) = write_store(dir, &mesh)?;
    Ok(json!({
        "path": path.display().to_string(),
        "mesh_hash": hash_hex(&mesh),
        "cells": mesh.n_cells(),
        "faces": mesh.n_faces(),
        "points": mesh.n_points(),
        "patches": mesh.patches().iter().map(|p| json!({"name": p.name, "kind": format!("{:?}", p.kind), "faces": p.size})).collect::<Vec<_>>(),
        "bounds": bounds(&mesh),
        "two_d": mesh.patches().iter().any(|p| p.kind == PatchKind::Empty),
        "adapt": adapt,
        "reused": reused,
    }))
}
