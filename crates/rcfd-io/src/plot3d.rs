//! Plot3D structured grids and function files (F-119, ADR 0011 section 2.3).
//!
//! Written from the public Plot3D description: a multi-block grid file holds the block count, the `ni nj nk` of
//! each block (`ni nj` in 2D) and then, block by block, all `x`, all `y`, all `z` (i fastest) and optionally an
//! integer `iblank` array. Formatted files are free-format text; unformatted files are Fortran sequential records
//! (each record is wrapped in a 4 byte length marker). The reader detects byte order from the first record marker and
//! single or double precision and the presence of `iblank` from the record length of the first coordinate record.
//!
//! Supported: multi-grid files with one record per block (the common "whole" layout), single-grid files
//! (the first record holds one `ni nj nk`) and binary files without record markers (`read_grid_raw`). Not supported: planar (one record for all blocks) layouts and
//! `iblank`-less formatted files whose token count does not identify the layout (ambiguous input is an error).

use crate::common::{build_mesh, SurfaceGroup};
use crate::guard::{check_count, check_file_size, mul, MAX_BLOCKS, MAX_ITEMS};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use std::fmt::Write as _;

/// One structured block. Points are stored with `i` fastest: `index = i + ni * (j + nj * k)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Points along i.
    pub ni: usize,
    /// Points along j.
    pub nj: usize,
    /// Points along k (1 for 2D grids).
    pub nk: usize,
    /// x coordinates.
    pub x: Vec<f64>,
    /// y coordinates.
    pub y: Vec<f64>,
    /// z coordinates (zeros for 2D grids).
    pub z: Vec<f64>,
    /// Optional iblank array.
    pub iblank: Option<Vec<i32>>,
}

impl Block {
    /// Number of points.
    pub fn n_points(&self) -> usize {
        self.ni * self.nj * self.nk
    }

    /// Point index.
    pub fn index(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.ni * (j + self.nj * k)
    }
}

/// A Plot3D grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    /// 2 or 3.
    pub dim: usize,
    /// Blocks.
    pub blocks: Vec<Block>,
}

/// Output layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Format {
    /// Fortran unformatted binary (true) or free-format text (false).
    pub binary: bool,
    /// Double precision (binary only; text is always written with full `f64` precision).
    pub double: bool,
    /// Big-endian record markers and data (binary only).
    pub big_endian: bool,
    /// Write the iblank array (the blocks must have it).
    pub iblank: bool,
    /// Write the leading block-count record (multi-grid file).
    pub multi_grid: bool,
}

impl Default for Format {
    fn default() -> Self {
        Format {
            binary: true,
            double: true,
            big_endian: false,
            iblank: false,
            multi_grid: true,
        }
    }
}

struct Rec<'a> {
    b: &'a [u8],
    pos: usize,
    be: bool,
    index: usize,
}

impl<'a> Rec<'a> {
    fn u32_at(&self, o: usize) -> Option<u32> {
        let s = self.b.get(o..o.checked_add(4)?)?;
        let a = [s[0], s[1], s[2], s[3]];
        Some(if self.be {
            u32::from_be_bytes(a)
        } else {
            u32::from_le_bytes(a)
        })
    }

    /// Next record's payload range.
    fn next(&mut self) -> Result<&'a [u8]> {
        self.index += 1;
        let at = self.pos;
        let len = self.u32_at(at).ok_or_else(|| {
            Error::Io(format!(
                "Plot3D: record {}: byte offset {at}: truncated length marker",
                self.index
            ))
        })? as usize;
        let start = at + 4;
        let end = start
            .checked_add(len)
            .filter(|&e| e.checked_add(4).is_some_and(|x| x <= self.b.len()))
            .ok_or_else(|| {
                Error::Io(format!(
                    "Plot3D: record {}: byte offset {at}: record of {len} bytes runs past the end of the file ({} bytes)",
                    self.index,
                    self.b.len()
                ))
            })?;
        if self.u32_at(end) != Some(len as u32) {
            return Err(Error::Io(format!(
                "Plot3D: record {}: byte offset {end}: trailing record marker does not match the leading one ({len})",
                self.index
            )));
        }
        self.pos = end + 4;
        Ok(&self.b[start..end])
    }
}

fn ints(b: &[u8], be: bool) -> Vec<i32> {
    b.chunks_exact(4)
        .map(|c| {
            let a = [c[0], c[1], c[2], c[3]];
            if be {
                i32::from_be_bytes(a)
            } else {
                i32::from_le_bytes(a)
            }
        })
        .collect()
}

fn floats(b: &[u8], double: bool, be: bool) -> Vec<f64> {
    if double {
        b.chunks_exact(8)
            .map(|c| {
                let a = [c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]];
                if be {
                    f64::from_be_bytes(a)
                } else {
                    f64::from_le_bytes(a)
                }
            })
            .collect()
    } else {
        b.chunks_exact(4)
            .map(|c| {
                let a = [c[0], c[1], c[2], c[3]];
                (if be {
                    f32::from_be_bytes(a)
                } else {
                    f32::from_le_bytes(a)
                }) as f64
            })
            .collect()
    }
}

fn dim_ok(v: i32, what: &str, rec: usize) -> Result<usize> {
    if v < 1 {
        return Err(Error::Io(format!(
            "Plot3D: record {rec}: {what} = {v} is not a positive size"
        )));
    }
    Ok(v as usize)
}

/// Total points of the dims, with the cap.
fn points_of(ni: usize, nj: usize, nk: usize, rec: usize) -> Result<usize> {
    let n = mul(mul(ni, nj, "Plot3D grid")?, nk, "Plot3D grid")?;
    if n > MAX_ITEMS {
        return Err(Error::Io(format!(
            "Plot3D: record {rec}: block of {n} points exceeds the limit of {MAX_ITEMS}"
        )));
    }
    Ok(n)
}

/// Detect the byte order from the first record.
fn detect_endian(b: &[u8]) -> Result<bool> {
    if b.len() < 8 {
        return Err(Error::Io("Plot3D: byte offset 0: file too short for a record".into()));
    }
    for be in [false, true] {
        let m = if be {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        } as usize;
        let ok = m >= 4 && m % 4 == 0 && m.checked_add(8).is_some_and(|t| t <= b.len());
        if ok {
            let tail = &b[4 + m..8 + m];
            let t = if be {
                u32::from_be_bytes([tail[0], tail[1], tail[2], tail[3]])
            } else {
                u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]])
            } as usize;
            if t == m {
                return Ok(be);
            }
        }
    }
    Err(Error::Io(
        "Plot3D: byte offset 0: not a Fortran unformatted file (the first record marker is not repeated after its data)".into(),
    ))
}

/// Read a binary (Fortran unformatted) grid file. `dim` 2 or 3 is detected from the dims record.
pub fn read_grid_binary(bytes: &[u8]) -> Result<Grid> {
    check_file_size(bytes.len(), "Plot3D")?;
    let be = detect_endian(bytes)?;
    let mut r = Rec {
        b: bytes,
        pos: 0,
        be,
        index: 0,
    };
    let first = r.next()?;
    let (nb, dims): (usize, Vec<i32>);
    if first.len() == 4 {
        nb = dim_ok(ints(first, be)[0], "number of blocks", 1)?;
        if nb > MAX_BLOCKS {
            return Err(Error::Io(format!(
                "Plot3D: record 1: {nb} blocks exceeds the limit of {MAX_BLOCKS}"
            )));
        }
        let d = r.next()?;
        dims = ints(d, be);
    } else {
        nb = 1;
        dims = ints(first, be);
    }
    let dim = if dims.len() == 3 * nb {
        3
    } else if dims.len() == 2 * nb {
        2
    } else {
        return Err(Error::Io(format!(
            "Plot3D: record {}: {} dimension values do not fit {nb} block(s) in 2D or 3D",
            r.index,
            dims.len()
        )));
    };
    let dim_rec = r.index;
    let mut blocks = Vec::new();
    let mut layout: Option<(bool, bool)> = None;
    for b in 0..nb {
        let ni = dim_ok(dims[dim * b], "ni", dim_rec)?;
        let nj = dim_ok(dims[dim * b + 1], "nj", dim_rec)?;
        let nk = if dim == 3 {
            dim_ok(dims[dim * b + 2], "nk", dim_rec)?
        } else {
            1
        };
        let n = points_of(ni, nj, nk, dim_rec)?;
        // cheap guard before the record is touched: the record must fit in the file
        check_count(
            n,
            4 * dim,
            bytes.len().saturating_sub(r.pos),
            &format!("Plot3D: block {}", b + 1),
        )?;
        let rec = r.next()?;
        let cands = [(false, false), (true, false), (false, true), (true, true)];
        let want = |(dbl, ib): (bool, bool)| n * dim * if dbl { 8 } else { 4 } + if ib { 4 * n } else { 0 };
        let found = cands.iter().copied().find(|c| want(*c) == rec.len());
        let (dbl, ib) = found.ok_or_else(|| {
            Error::Io(format!(
                "Plot3D: record {}: {} bytes fits no single/double, iblank/none layout of a {ni} x {nj} x {nk} block",
                r.index,
                rec.len()
            ))
        })?;
        match layout {
            None => layout = Some((dbl, ib)),
            Some(l) if l != (dbl, ib) => {
                return Err(Error::Io(format!(
                    "Plot3D: record {}: layout differs from the first block",
                    r.index
                )))
            }
            _ => {}
        }
        let cb = n * dim * if dbl { 8 } else { 4 };
        let v = floats(&rec[..cb], dbl, be);
        let iblank = if ib { Some(ints(&rec[cb..], be)) } else { None };
        blocks.push(Block {
            ni,
            nj,
            nk,
            x: v[..n].to_vec(),
            y: v[n..2 * n].to_vec(),
            z: if dim == 3 {
                v[2 * n..3 * n].to_vec()
            } else {
                vec![0.0; n]
            },
            iblank,
        });
    }
    if r.pos != bytes.len() {
        return Err(Error::Io(format!(
            "Plot3D: byte offset {}: {} unread bytes after the last block",
            r.pos,
            bytes.len() - r.pos
        )));
    }
    Ok(Grid { dim, blocks })
}

/// Read a formatted (text) grid file. `iblank` is detected from the number of values; `multi_grid` tells whether the
/// file starts with a block count (otherwise the first line holds one `ni nj nk`).
pub fn read_grid_ascii(text: &str, multi_grid: bool, dim: usize) -> Result<Grid> {
    check_file_size(text.len(), "Plot3D")?;
    if dim != 2 && dim != 3 {
        return Err(Error::Io("Plot3D: dim must be 2 or 3".into()));
    }
    let mut toks: Vec<(&str, usize)> = Vec::new();
    for (ln, l) in text.lines().enumerate() {
        for t in l.split_ascii_whitespace() {
            toks.push((t, ln + 1));
            if toks.len() > 2 * MAX_ITEMS {
                return Err(Error::Io("Plot3D: too many values".into()));
            }
        }
    }
    let mut p = 0;
    let int = |toks: &[(&str, usize)], p: usize, what: &str| -> Result<usize> {
        let (t, ln) = *toks.get(p).ok_or_else(|| {
            Error::Io(format!(
                "Plot3D: line {}: unexpected end of file reading {what}",
                toks.last().map_or(1, |x| x.1)
            ))
        })?;
        t.parse::<usize>()
            .ok()
            .filter(|&v| v >= 1)
            .ok_or_else(|| Error::Io(format!("Plot3D: line {ln}: '{t}' is not a positive integer ({what})")))
    };
    let nb = if multi_grid {
        p += 1;
        int(&toks, 0, "number of blocks")?
    } else {
        1
    };
    if nb > MAX_BLOCKS {
        return Err(Error::Io(format!(
            "Plot3D: {nb} blocks exceeds the limit of {MAX_BLOCKS}"
        )));
    }
    let mut dims = Vec::new();
    for _ in 0..nb {
        let ni = int(&toks, p, "ni")?;
        let nj = int(&toks, p + 1, "nj")?;
        let nk = if dim == 3 { int(&toks, p + 2, "nk")? } else { 1 };
        p += dim;
        dims.push((ni, nj, nk, points_of(ni, nj, nk, 0)?));
    }
    let total: usize = dims.iter().map(|d| d.3).sum();
    let have = toks.len() - p;
    let (with_ib, plain) = (total * (dim + 1), total * dim);
    let ib = if have == plain {
        false
    } else if have == with_ib {
        true
    } else {
        return Err(Error::Io(format!(
            "Plot3D: {have} values follow the dimensions; expected {plain} (coordinates) or {with_ib} (with iblank)"
        )));
    };
    let mut blocks = Vec::new();
    for (ni, nj, nk, n) in dims {
        let mut take = |count: usize| -> Result<Vec<f64>> {
            let mut v = Vec::with_capacity(count);
            for &(t, ln) in &toks[p..p + count] {
                let x: f64 = t
                    .parse()
                    .or_else(|_| t.replace(['d', 'D'], "e").parse())
                    .map_err(|_| Error::Io(format!("Plot3D: line {ln}: '{t}' is not a number")))?;
                if !x.is_finite() {
                    return Err(Error::Io(format!("Plot3D: line {ln}: non-finite number")));
                }
                v.push(x);
            }
            p += count;
            Ok(v)
        };
        let x = take(n)?;
        let y = take(n)?;
        let z = if dim == 3 { take(n)? } else { vec![0.0; n] };
        let iblank = if ib {
            let v = take(n)?;
            Some(v.iter().map(|&a| a as i32).collect())
        } else {
            None
        };
        blocks.push(Block {
            ni,
            nj,
            nk,
            x,
            y,
            z,
            iblank,
        });
    }
    Ok(Grid { dim, blocks })
}

fn push_rec(out: &mut Vec<u8>, payload: &[u8], be: bool) {
    let m = if be {
        (payload.len() as u32).to_be_bytes()
    } else {
        (payload.len() as u32).to_le_bytes()
    };
    out.extend_from_slice(&m);
    out.extend_from_slice(payload);
    out.extend_from_slice(&m);
}

fn put_f(buf: &mut Vec<u8>, v: f64, f: &Format) {
    match (f.double, f.big_endian) {
        (true, false) => buf.extend_from_slice(&v.to_le_bytes()),
        (true, true) => buf.extend_from_slice(&v.to_be_bytes()),
        (false, false) => buf.extend_from_slice(&(v as f32).to_le_bytes()),
        (false, true) => buf.extend_from_slice(&(v as f32).to_be_bytes()),
    }
}

fn put_i(buf: &mut Vec<u8>, v: i32, be: bool) {
    buf.extend_from_slice(&if be { v.to_be_bytes() } else { v.to_le_bytes() });
}

fn check_grid(g: &Grid, f: &Format) -> Result<()> {
    if g.dim != 2 && g.dim != 3 {
        return Err(Error::Io("Plot3D: dim must be 2 or 3".into()));
    }
    if !f.multi_grid && g.blocks.len() != 1 {
        return Err(Error::Io("Plot3D: a single-grid file holds exactly one block".into()));
    }
    for (i, b) in g.blocks.iter().enumerate() {
        let n = b.n_points();
        if b.x.len() != n || b.y.len() != n || b.z.len() != n {
            return Err(Error::Io(format!(
                "Plot3D: block {}: coordinate arrays do not match ni*nj*nk",
                i + 1
            )));
        }
        if g.dim == 2 && b.nk != 1 {
            return Err(Error::Io(format!("Plot3D: block {}: a 2D grid needs nk = 1", i + 1)));
        }
        if f.iblank && b.iblank.as_ref().map(|v| v.len()) != Some(n) {
            return Err(Error::Io(format!(
                "Plot3D: block {}: iblank requested but missing",
                i + 1
            )));
        }
        let all = b.x.iter().chain(&b.y).chain(&b.z);
        if all.into_iter().any(|v| !v.is_finite()) {
            return Err(Error::Io(format!("Plot3D: block {}: non-finite coordinate", i + 1)));
        }
    }
    Ok(())
}

/// Serialise a grid. Formatted output writes one value per `ni` run per line with full precision.
pub fn write_grid(g: &Grid, f: &Format) -> Result<Vec<u8>> {
    check_grid(g, f)?;
    if f.binary {
        let be = f.big_endian;
        let mut out = Vec::new();
        if f.multi_grid {
            let mut p = Vec::new();
            put_i(&mut p, g.blocks.len() as i32, be);
            push_rec(&mut out, &p, be);
        }
        let mut p = Vec::new();
        for b in &g.blocks {
            put_i(&mut p, b.ni as i32, be);
            put_i(&mut p, b.nj as i32, be);
            if g.dim == 3 {
                put_i(&mut p, b.nk as i32, be);
            }
        }
        push_rec(&mut out, &p, be);
        for b in &g.blocks {
            let mut p = Vec::new();
            let comps: Vec<&Vec<f64>> = if g.dim == 3 {
                vec![&b.x, &b.y, &b.z]
            } else {
                vec![&b.x, &b.y]
            };
            for c in comps {
                for &v in c {
                    put_f(&mut p, v, f);
                }
            }
            if f.iblank {
                for &v in b.iblank.as_ref().expect("checked") {
                    put_i(&mut p, v, be);
                }
            }
            push_rec(&mut out, &p, be);
        }
        Ok(out)
    } else {
        let mut o = String::new();
        if f.multi_grid {
            let _ = writeln!(o, "{}", g.blocks.len());
        }
        for b in &g.blocks {
            if g.dim == 3 {
                let _ = writeln!(o, "{} {} {}", b.ni, b.nj, b.nk);
            } else {
                let _ = writeln!(o, "{} {}", b.ni, b.nj);
            }
        }
        for b in &g.blocks {
            let comps: Vec<&Vec<f64>> = if g.dim == 3 {
                vec![&b.x, &b.y, &b.z]
            } else {
                vec![&b.x, &b.y]
            };
            for c in comps {
                for row in c.chunks(b.ni.max(1)) {
                    let s: Vec<String> = row.iter().map(|v| format!("{v:?}")).collect();
                    let _ = writeln!(o, "{}", s.join(" "));
                }
            }
            if f.iblank {
                for row in b.iblank.as_ref().expect("checked").chunks(b.ni.max(1)) {
                    let s: Vec<String> = row.iter().map(|v| v.to_string()).collect();
                    let _ = writeln!(o, "{}", s.join(" "));
                }
            }
        }
        Ok(o.into_bytes())
    }
}

/// Read a binary grid without Fortran record markers ("C binary", as in the public NASA blunt-fin sample): the
/// integers `[nblocks] ni nj [nk]` per block, then each block's coordinates (and optional iblank) back to back. Byte
/// order, single or double precision, 2D or 3D, multi-grid or single-grid and iblank are found by requiring that the
/// declared sizes account for the file length exactly; if more than one reading fits, the first in the order
/// little-endian, big-endian / single-grid, multi-grid / 3D, 2D / single, double / no iblank, iblank wins.
pub fn read_grid_raw(bytes: &[u8]) -> Result<Grid> {
    check_file_size(bytes.len(), "Plot3D")?;
    let int_at = |o: usize, be: bool| -> Option<i32> {
        let s = bytes.get(o..o.checked_add(4)?)?;
        let a = [s[0], s[1], s[2], s[3]];
        Some(if be {
            i32::from_be_bytes(a)
        } else {
            i32::from_le_bytes(a)
        })
    };
    for be in [false, true] {
        for multi in [false, true] {
            for dim in [3usize, 2] {
                let mut o = 0;
                let nb = if multi {
                    let Some(v) = int_at(0, be).filter(|&v| v >= 1 && v as usize <= MAX_BLOCKS) else {
                        continue;
                    };
                    o = 4;
                    v as usize
                } else {
                    1
                };
                if o + 4 * dim * nb > bytes.len() {
                    continue;
                }
                let mut dims = Vec::new();
                let mut ok = true;
                let mut points = 0usize;
                for b in 0..nb {
                    let mut d = [1usize; 3];
                    for (c, slot) in d.iter_mut().enumerate().take(dim) {
                        match int_at(o + 4 * (dim * b + c), be) {
                            Some(v) if v >= 1 => *slot = v as usize,
                            _ => ok = false,
                        }
                    }
                    if !ok {
                        break;
                    }
                    let n = match d[0].checked_mul(d[1]).and_then(|x| x.checked_mul(d[2])) {
                        Some(n) if n <= MAX_ITEMS => n,
                        _ => {
                            ok = false;
                            break;
                        }
                    };
                    points += n;
                    if points > MAX_ITEMS {
                        ok = false;
                        break;
                    }
                    dims.push((d, n));
                }
                if !ok {
                    continue;
                }
                let head = o + 4 * dim * nb;
                for (double, ib) in [(false, false), (true, false), (false, true), (true, true)] {
                    let per = points * dim * if double { 8 } else { 4 } + if ib { 4 * points } else { 0 };
                    if head + per != bytes.len() {
                        continue;
                    }
                    let mut pos = head;
                    let mut blocks = Vec::new();
                    for (d, n) in &dims {
                        let cb = n * dim * if double { 8 } else { 4 };
                        let v = floats(&bytes[pos..pos + cb], double, be);
                        pos += cb;
                        let iblank = if ib {
                            let r = ints(&bytes[pos..pos + 4 * n], be);
                            pos += 4 * n;
                            Some(r)
                        } else {
                            None
                        };
                        if v.iter().any(|x| !x.is_finite()) {
                            return Err(Error::Io(format!(
                                "Plot3D: byte offset {}: non-finite coordinate in a block of the headerless layout",
                                pos
                            )));
                        }
                        blocks.push(Block {
                            ni: d[0],
                            nj: d[1],
                            nk: d[2],
                            x: v[..*n].to_vec(),
                            y: v[*n..2 * n].to_vec(),
                            z: if dim == 3 {
                                v[2 * n..3 * n].to_vec()
                            } else {
                                vec![0.0; *n]
                            },
                            iblank,
                        });
                    }
                    return Ok(Grid { dim, blocks });
                }
            }
        }
    }
    Err(Error::Io(
        "Plot3D: byte offset 0: the declared block sizes do not account for the file length in any supported layout"
            .into(),
    ))
}

/// Read a grid file, deciding text or binary: text if it starts with a digit after white space and does not look like
/// a Fortran record.
pub fn read_grid(bytes: &[u8]) -> Result<Grid> {
    if detect_endian(bytes).is_ok() {
        return read_grid_binary(bytes);
    }
    if let Ok(g) = read_grid_raw(bytes) {
        return Ok(g);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| Error::Io("Plot3D: byte offset 0: neither a valid Fortran unformatted file nor text".into()))?;
    // multi-grid if the first line holds one integer and the dims follow, otherwise single
    let first_line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let n_first = first_line.split_ascii_whitespace().count();
    let multi = n_first == 1;
    let dim = if multi {
        // number of dims per block on the second line
        let second = text.lines().filter(|l| !l.trim().is_empty()).nth(1).unwrap_or("");
        if second.split_ascii_whitespace().count() == 2 {
            2
        } else {
            3
        }
    } else if n_first == 2 {
        2
    } else {
        3
    };
    read_grid_ascii(text, multi, dim)
}

/// Read a grid from disk.
pub fn read_file(path: impl AsRef<std::path::Path>) -> Result<Grid> {
    let path = path.as_ref();
    check_file_size(std::fs::metadata(path)?.len() as usize, "Plot3D")?;
    read_grid(&std::fs::read(path)?).map_err(|e| Error::Io(format!("{}: {e}", path.display())))
}

/// A function (solution) file: per block `nvar` arrays of `ni*nj*nk` values.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionFile {
    /// Per block: (ni, nj, nk, arrays).
    pub blocks: Vec<(usize, usize, usize, Vec<Vec<f64>>)>,
}

/// Write a binary multi-block function file.
pub fn write_function_binary(f: &FunctionFile, fmt: &Format) -> Result<Vec<u8>> {
    let be = fmt.big_endian;
    let mut out = Vec::new();
    let mut p = Vec::new();
    put_i(&mut p, f.blocks.len() as i32, be);
    push_rec(&mut out, &p, be);
    let mut p = Vec::new();
    for (ni, nj, nk, vars) in &f.blocks {
        for v in [*ni, *nj, *nk, vars.len()] {
            put_i(&mut p, v as i32, be);
        }
    }
    push_rec(&mut out, &p, be);
    for (ni, nj, nk, vars) in &f.blocks {
        let n = ni * nj * nk;
        let mut p = Vec::new();
        for v in vars {
            if v.len() != n {
                return Err(Error::Io(
                    "Plot3D function: array length does not match ni*nj*nk".into(),
                ));
            }
            for &x in v {
                put_f(&mut p, x, fmt);
            }
        }
        push_rec(&mut out, &p, be);
    }
    Ok(out)
}

/// Read a binary multi-block function file.
pub fn read_function_binary(bytes: &[u8]) -> Result<FunctionFile> {
    check_file_size(bytes.len(), "Plot3D function")?;
    let be = detect_endian(bytes)?;
    let mut r = Rec {
        b: bytes,
        pos: 0,
        be,
        index: 0,
    };
    let nbr = r.next()?;
    if nbr.len() != 4 {
        return Err(Error::Io("Plot3D function: record 1 must hold the block count".into()));
    }
    let nb = dim_ok(ints(nbr, be)[0], "number of blocks", 1)?;
    if nb > MAX_BLOCKS {
        return Err(Error::Io(format!(
            "Plot3D function: {nb} blocks exceeds the limit of {MAX_BLOCKS}"
        )));
    }
    let d = ints(r.next()?, be);
    if d.len() != 4 * nb {
        return Err(Error::Io(format!(
            "Plot3D function: record 2 has {} values, expected {} (ni nj nk nvar per block)",
            d.len(),
            4 * nb
        )));
    }
    let mut blocks = Vec::new();
    for b in 0..nb {
        let ni = dim_ok(d[4 * b], "ni", 2)?;
        let nj = dim_ok(d[4 * b + 1], "nj", 2)?;
        let nk = dim_ok(d[4 * b + 2], "nk", 2)?;
        let nv = dim_ok(d[4 * b + 3], "nvar", 2)?;
        let n = points_of(ni, nj, nk, 2)?;
        let total = mul(n, nv, "Plot3D function")?;
        check_count(total, 4, bytes.len().saturating_sub(r.pos), "Plot3D function block")?;
        let rec = r.next()?;
        let dbl = if rec.len() == total * 8 {
            true
        } else if rec.len() == total * 4 {
            false
        } else {
            return Err(Error::Io(format!(
                "Plot3D function: record {}: {} bytes fits neither single nor double precision for {nv} variables of {n} points",
                r.index,
                rec.len()
            )));
        };
        let v = floats(rec, dbl, be);
        blocks.push((ni, nj, nk, v.chunks(n).map(|c| c.to_vec()).collect()));
    }
    Ok(FunctionFile { blocks })
}

/// Hexahedral mesh of one 3D block with patches `imin imax jmin jmax kmin kmax` (wall kind by default). iblank is not
/// interpreted.
pub fn block_to_mesh(g: &Grid, block: usize) -> Result<Mesh> {
    if g.dim != 3 {
        return Err(Error::Mesh("Plot3D: only 3D blocks convert to a volume mesh".into()));
    }
    let b = g
        .blocks
        .get(block)
        .ok_or_else(|| Error::Mesh(format!("Plot3D: no block {}", block + 1)))?;
    if b.ni < 2 || b.nj < 2 || b.nk < 2 {
        return Err(Error::Mesh(
            "Plot3D: a block needs at least 2 points in every direction".into(),
        ));
    }
    let pts: Vec<Vec3> = (0..b.n_points()).map(|n| [b.x[n], b.y[n], b.z[n]]).collect();
    let id = |i, j, k| b.index(i, j, k);
    let mut cells = Vec::new();
    for k in 0..b.nk - 1 {
        for j in 0..b.nj - 1 {
            for i in 0..b.ni - 1 {
                let n = [
                    id(i, j, k),
                    id(i + 1, j, k),
                    id(i + 1, j + 1, k),
                    id(i, j + 1, k),
                    id(i, j, k + 1),
                    id(i + 1, j, k + 1),
                    id(i + 1, j + 1, k + 1),
                    id(i, j + 1, k + 1),
                ];
                let tpl = crate::common::cell_face_template(8, crate::common::Shape::Hex).expect("hex template");
                cells.push(
                    tpl.iter()
                        .map(|f| f.iter().map(|&q| n[q]).collect::<Vec<usize>>())
                        .collect(),
                );
            }
        }
    }
    let mut groups = Vec::new();
    let mut quad = |name: &str, f: Vec<Vec<usize>>| {
        groups.push(SurfaceGroup {
            name: name.to_string(),
            faces: f,
            kind: Some(rcfd_mesh::PatchKind::Wall),
        })
    };
    let (ni, nj, nk) = (b.ni, b.nj, b.nk);
    let face_set = |fix: usize, side: usize| -> Vec<Vec<usize>> {
        let mut v = Vec::new();
        match fix {
            0 => {
                let i = if side == 0 { 0 } else { ni - 1 };
                for k in 0..nk - 1 {
                    for j in 0..nj - 1 {
                        v.push(vec![id(i, j, k), id(i, j + 1, k), id(i, j + 1, k + 1), id(i, j, k + 1)]);
                    }
                }
            }
            1 => {
                let j = if side == 0 { 0 } else { nj - 1 };
                for k in 0..nk - 1 {
                    for i in 0..ni - 1 {
                        v.push(vec![id(i, j, k), id(i + 1, j, k), id(i + 1, j, k + 1), id(i, j, k + 1)]);
                    }
                }
            }
            _ => {
                let k = if side == 0 { 0 } else { nk - 1 };
                for j in 0..nj - 1 {
                    for i in 0..ni - 1 {
                        v.push(vec![id(i, j, k), id(i + 1, j, k), id(i + 1, j + 1, k), id(i, j + 1, k)]);
                    }
                }
            }
        }
        v
    };
    for (name, fix, side) in [
        ("imin", 0, 0),
        ("imax", 0, 1),
        ("jmin", 1, 0),
        ("jmax", 1, 1),
        ("kmin", 2, 0),
        ("kmax", 2, 1),
    ] {
        let f = face_set(fix, side);
        quad(name, f);
    }
    build_mesh(pts, cells, groups, Vec::new())
}
