//! Time-series field store of a run (Studio plan L3, ADR 0124): `runs/<run-id>/fields/`.
//!
//! One append-only file `store.rts` holds the mesh once (with its content hash) and one snapshot per saved time.
//! A snapshot is a set of named scalar fields, each with a unit string and a location (cell or point). Every
//! field is cut into fixed-size blocks so one field, or a subset of cells, is read without touching the rest.
//!
//! ```text
//! header   "RCFDTSS\0" | u32 version | u64 fnv64(previous 12 bytes)                      20 bytes
//! frame    kind[4] | u64 len | u64 fnv64(payload) | u64 fnv64(previous 20 bytes) | payload
//!            MESH  u64 mesh_hash | u64 n_cells | u64 n_points | rcfd_io::container::mesh_to_bytes
//!            DATA  one block of one field (raw little-endian, or byte-shuffled then deflated)
//!            SNAP  JSON directory of one snapshot: step, time, fields (unit, location, block offsets)
//!            FOOT  JSON index of all snapshots (written by `Writer::finish` only)
//! trailer  u64 FOOT offset | "RTSE" | u64 fnv64(previous 12 bytes)                       20 bytes
//! ```
//!
//! Crash safety: a snapshot exists once its SNAP frame is complete. The data blocks are written before it, so a
//! writer killed mid-snapshot leaves only an orphan tail; a reader without a valid footer rebuilds the index by
//! scanning frame heads and reports [`IndexSource::Scanned`]. Every byte of the file is covered by a checksum
//! (header, frame head, payload, trailer). Access is seek-based: memory mapping needs `unsafe`, which this
//! workspace forbids. `index.json` next to the store is an advisory mirror written by `finish`; readers never
//! need it.

use rcfd_core::hash::fnv64;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Name of the store file inside the fields directory.
pub const STORE_FILE: &str = "store.rts";
/// Name of the advisory JSON index written next to it by [`Writer::finish`].
pub const INDEX_FILE: &str = "index.json";
/// File magic.
pub const MAGIC: [u8; 8] = *b"RCFDTSS\0";
/// Format version written by this build (files with a larger version are refused).
pub const VERSION: u32 = 1;
/// Bytes of the file header.
pub const HEADER_LEN: u64 = 20;
/// Bytes of a frame head.
pub const FRAME_HEAD_LEN: u64 = 28;
/// Bytes of the trailer.
pub const TRAILER_LEN: u64 = 20;
/// Values per block unless [`WriteOptions::block_values`] says otherwise (512 KiB of f64).
pub const DEFAULT_BLOCK_VALUES: usize = 1 << 16;
const MAX_BLOCK_VALUES: usize = 1 << 24;
const MAX_JSON_FRAME: u64 = 1 << 28;
const DEFLATE_LEVEL: u8 = 1;
const TRAILER_MAGIC: [u8; 4] = *b"RTSE";
const K_MESH: [u8; 4] = *b"MESH";
const K_DATA: [u8; 4] = *b"DATA";
const K_SNAP: [u8; 4] = *b"SNAP";
const K_FOOT: [u8; 4] = *b"FOOT";

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Io(format!("tsstore: {}", msg.into())))
}

/// Where the values of a field live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Location {
    /// One value per cell.
    Cell,
    /// One value per mesh point.
    Point,
}

/// Storage precision of a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    /// IEEE double, bit-exact.
    F64,
    /// Lossy: rounded to the nearest IEEE single; error bound [`f32_error_bound`].
    F32,
}

/// Block encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    /// Little-endian values as they are.
    Raw,
    /// Lossless: bytes regrouped by significance (all first bytes, then all second bytes, ...) and deflated.
    ShuffleDeflate,
}

/// Options of a writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOptions {
    /// Values per block (1 ..= 2^24); the unit of partial reads.
    pub block_values: usize,
    /// Block encoding.
    pub codec: Codec,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            block_values: DEFAULT_BLOCK_VALUES,
            codec: Codec::Raw,
        }
    }
}

/// Largest |stored - x| of the lossy single-precision mode: half an f32 ulp relative (2^-24 |x|) in the normal
/// range plus half the smallest f32 subnormal spacing (2^-150) below it. Exact for NaN-free round trips of F64.
pub fn f32_error_bound(x: f64) -> f64 {
    x.abs() * 2f64.powi(-24) + 2f64.powi(-150)
}

/// Description of one stored field of one snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldInfo {
    /// Field name (unique within the snapshot).
    pub name: String,
    /// Unit as given by the writer (`"unknown"` when the writer did not know it).
    pub unit: String,
    /// Cell or point values.
    pub location: Location,
    /// Number of values.
    pub n: usize,
    /// Storage precision.
    pub precision: Precision,
    /// Block encoding.
    pub codec: Codec,
    /// Values per block.
    pub block_values: usize,
    /// File offsets of the DATA frames, one per block.
    pub blocks: Vec<u64>,
}

impl FieldInfo {
    /// Registered bound of |read - written| for a written value `x` (0 for [`Precision::F64`]).
    pub fn error_bound(&self, x: f64) -> f64 {
        match self.precision {
            Precision::F64 => 0.0,
            Precision::F32 => f32_error_bound(x),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapDir {
    step: u64,
    time: f64,
    fields: Vec<FieldInfo>,
}

/// Step, time and file offset of one committed snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapInfo {
    /// Solver step or iteration.
    pub step: u64,
    /// Simulated time.
    pub time: f64,
    offset: u64,
    end: u64,
}

impl SnapInfo {
    /// File length at which this snapshot is complete (just after its directory frame).
    pub fn end_offset(&self) -> u64 {
        self.end
    }
}

#[derive(Serialize, Deserialize)]
struct Footer {
    mesh: u64,
    snapshots: Vec<(u64, f64, u64, u64)>,
}

/// Where a reader's snapshot index came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexSource {
    /// The footer written by [`Writer::finish`] (all checksums matched).
    Footer,
    /// Rebuilt by scanning frame heads (live or interrupted writer, or a footer that failed its checks).
    Scanned {
        /// Why the footer was not used.
        reason: String,
        /// Bytes after the last complete snapshot (an interrupted snapshot, a footer, a trailer).
        tail_bytes: u64,
    },
}

// ---------------------------------------------------------------- low-level framing

fn head_bytes(kind: [u8; 4], payload: &[u8]) -> [u8; 28] {
    let mut h = [0u8; 28];
    h[..4].copy_from_slice(&kind);
    h[4..12].copy_from_slice(&(payload.len() as u64).to_le_bytes());
    h[12..20].copy_from_slice(&fnv64(payload).to_le_bytes());
    let s = fnv64(&h[..20]);
    h[20..].copy_from_slice(&s.to_le_bytes());
    h
}

fn le64(b: &[u8]) -> u64 {
    u64::from_le_bytes(b[..8].try_into().expect("8 bytes"))
}

#[derive(Debug, Clone, Copy)]
struct Head {
    kind: [u8; 4],
    len: u64,
    sum: u64,
}

fn read_at(f: &mut File, off: u64, buf: &mut [u8]) -> Result<()> {
    f.seek(SeekFrom::Start(off))?;
    f.read_exact(buf)?;
    Ok(())
}

/// Frame head at `off`; `Ok(None)` when the head or its payload runs past `len` (a truncated tail).
fn head_at(f: &mut File, off: u64, len: u64) -> Result<Option<Head>> {
    if len < off || len - off < FRAME_HEAD_LEN {
        return Ok(None);
    }
    let mut h = [0u8; 28];
    read_at(f, off, &mut h)?;
    if fnv64(&h[..20]) != le64(&h[20..]) {
        return bad(format!("frame head checksum mismatch at offset {off}"));
    }
    let head = Head {
        kind: h[..4].try_into().expect("4 bytes"),
        len: le64(&h[4..]),
        sum: le64(&h[12..]),
    };
    if head.len > len - off - FRAME_HEAD_LEN {
        return Ok(None);
    }
    Ok(Some(head))
}

/// Complete, verified payload of a frame of `kind` at `off`.
fn payload_at(f: &mut File, off: u64, len: u64, kind: [u8; 4], max: u64) -> Result<Vec<u8>> {
    let Some(h) = head_at(f, off, len)? else {
        return bad(format!("frame at offset {off} runs past the end of the file"));
    };
    if h.kind != kind {
        return bad(format!(
            "expected a {} frame at offset {off}",
            String::from_utf8_lossy(&kind)
        ));
    }
    if h.len > max {
        return bad(format!("frame at offset {off} declares {} bytes", h.len));
    }
    let mut p = vec![0u8; h.len as usize];
    read_at(f, off + FRAME_HEAD_LEN, &mut p)?;
    if fnv64(&p) != h.sum {
        return bad(format!("payload checksum mismatch in frame at offset {off}"));
    }
    Ok(p)
}

fn header_bytes() -> [u8; 20] {
    let mut h = [0u8; 20];
    h[..8].copy_from_slice(&MAGIC);
    h[8..12].copy_from_slice(&VERSION.to_le_bytes());
    let s = fnv64(&h[..12]);
    h[12..].copy_from_slice(&s.to_le_bytes());
    h
}

fn check_header(f: &mut File, len: u64) -> Result<()> {
    if len < HEADER_LEN {
        return bad("file shorter than the header");
    }
    let mut h = [0u8; 20];
    read_at(f, 0, &mut h)?;
    if h[..8] != MAGIC {
        return bad("not an rcfd time-series store (bad magic)");
    }
    if fnv64(&h[..12]) != le64(&h[12..]) {
        return bad("header checksum mismatch");
    }
    let v = u32::from_le_bytes(h[8..12].try_into().expect("4 bytes"));
    if v == 0 || v > VERSION {
        return bad(format!(
            "unsupported store version {v} (this build reads up to {VERSION})"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct MeshPrefix {
    end: u64,
    hash: u64,
    n_cells: usize,
    n_points: usize,
}

fn mesh_prefix(f: &mut File, len: u64) -> Result<MeshPrefix> {
    let Some(h) = head_at(f, HEADER_LEN, len)? else {
        return bad("the mesh frame is incomplete (the writer stopped before the mesh was written)");
    };
    if h.kind != K_MESH || h.len < 24 {
        return bad("the first frame is not a mesh frame");
    }
    let mut p = [0u8; 24];
    read_at(f, HEADER_LEN + FRAME_HEAD_LEN, &mut p)?;
    Ok(MeshPrefix {
        end: HEADER_LEN + FRAME_HEAD_LEN + h.len,
        hash: le64(&p),
        n_cells: le64(&p[8..]) as usize,
        n_points: le64(&p[16..]) as usize,
    })
}

fn check_dir(d: &SnapDir, m: &MeshPrefix, snap_off: u64) -> Result<()> {
    if !d.time.is_finite() {
        return bad("snapshot time is not finite");
    }
    for (i, f) in d.fields.iter().enumerate() {
        let want = match f.location {
            Location::Cell => m.n_cells,
            Location::Point => m.n_points,
        };
        if f.n != want || f.block_values == 0 || f.block_values > MAX_BLOCK_VALUES {
            return bad(format!(
                "field '{}' of step {} has an inconsistent size",
                f.name, d.step
            ));
        }
        if f.blocks.len() != f.n.div_ceil(f.block_values) || f.blocks.iter().any(|&b| b < m.end || b >= snap_off) {
            return bad(format!(
                "field '{}' of step {} has an inconsistent block list",
                f.name, d.step
            ));
        }
        if d.fields[..i].iter().any(|g| g.name == f.name) {
            return bad(format!("field '{}' appears twice in step {}", f.name, d.step));
        }
    }
    Ok(())
}

fn monotone(prev: Option<&SnapInfo>, step: u64, time: f64) -> bool {
    prev.map_or(true, |p| step > p.step && time >= p.time)
}

struct ScanOut {
    snaps: Vec<SnapInfo>,
    committed_end: u64,
}

/// Scan frames from `start` (just after the mesh frame or the last committed snapshot).
fn scan(f: &mut File, len: u64, m: &MeshPrefix, start: u64, prev: Option<SnapInfo>) -> Result<ScanOut> {
    let mut out = ScanOut {
        snaps: Vec::new(),
        committed_end: start,
    };
    let mut pos = start;
    while let Some(h) = head_at(f, pos, len)? {
        let next = pos + FRAME_HEAD_LEN + h.len;
        match h.kind {
            K_DATA | K_FOOT => {}
            K_SNAP => {
                let p = payload_at(f, pos, len, K_SNAP, MAX_JSON_FRAME)?;
                let d: SnapDir =
                    serde_json::from_slice(&p).or_else(|e| bad(format!("snapshot at offset {pos}: {e}")))?;
                check_dir(&d, m, pos)?;
                if !monotone(out.snaps.last().or(prev.as_ref()), d.step, d.time) {
                    return bad(format!(
                        "snapshot at offset {pos} does not follow its predecessor in step and time"
                    ));
                }
                out.snaps.push(SnapInfo {
                    step: d.step,
                    time: d.time,
                    offset: pos,
                    end: next,
                });
                out.committed_end = next;
            }
            k => {
                return bad(format!(
                    "unexpected {} frame at offset {pos}",
                    String::from_utf8_lossy(&k)
                ))
            }
        }
        pos = next;
    }
    Ok(out)
}

fn read_footer(f: &mut File, len: u64, m: &MeshPrefix) -> std::result::Result<Vec<SnapInfo>, String> {
    if len < m.end + FRAME_HEAD_LEN + TRAILER_LEN {
        return Err("no footer (the writer has not finished: live or interrupted run)".into());
    }
    let mut t = [0u8; 20];
    read_at(f, len - TRAILER_LEN, &mut t).map_err(|e| e.to_string())?;
    if t[8..12] != TRAILER_MAGIC || fnv64(&t[..12]) != le64(&t[12..]) {
        return Err(
            "no valid trailer (the writer has not finished: live or interrupted run, or a damaged trailer)".into(),
        );
    }
    let off = le64(&t);
    if off < m.end || off > len - TRAILER_LEN - FRAME_HEAD_LEN {
        return Err("trailer points outside the file".into());
    }
    let body_len = len - TRAILER_LEN;
    let p = payload_at(f, off, body_len, K_FOOT, MAX_JSON_FRAME).map_err(|e| e.to_string())?;
    if off + FRAME_HEAD_LEN + p.len() as u64 != body_len {
        return Err("footer frame does not end at the trailer".into());
    }
    let foot: Footer = serde_json::from_slice(&p).map_err(|e| format!("footer: {e}"))?;
    if foot.mesh != HEADER_LEN {
        return Err("footer names a different mesh frame".into());
    }
    let mut snaps: Vec<SnapInfo> = Vec::with_capacity(foot.snapshots.len());
    for (step, time, offset, end) in foot.snapshots {
        let lo = snaps.last().map_or(m.end, |s| s.end);
        if !time.is_finite() || !monotone(snaps.last(), step, time) || offset < lo || end <= offset || end > off {
            return Err("footer index is not consistent".into());
        }
        snaps.push(SnapInfo {
            step,
            time,
            offset,
            end,
        });
    }
    Ok(snaps)
}

// ---------------------------------------------------------------- block codec

fn encode(v: &[f64], p: Precision, c: Codec) -> Vec<u8> {
    let raw: Vec<u8> = match p {
        Precision::F64 => v.iter().flat_map(|x| x.to_bits().to_le_bytes()).collect(),
        Precision::F32 => v.iter().flat_map(|x| (*x as f32).to_bits().to_le_bytes()).collect(),
    };
    match c {
        Codec::Raw => raw,
        Codec::ShuffleDeflate => {
            let (n, w) = (v.len(), raw.len() / v.len().max(1));
            let mut s = vec![0u8; raw.len()];
            for i in 0..n {
                for j in 0..w {
                    s[j * n + i] = raw[i * w + j];
                }
            }
            miniz_oxide::deflate::compress_to_vec(&s, DEFLATE_LEVEL)
        }
    }
}

fn decode(payload: &[u8], count: usize, p: Precision, c: Codec) -> Result<Vec<f64>> {
    let w = match p {
        Precision::F64 => 8,
        Precision::F32 => 4,
    };
    let raw_len = count * w;
    let raw: Cow<[u8]> = match c {
        Codec::Raw => Cow::Borrowed(payload),
        Codec::ShuffleDeflate => {
            let s = miniz_oxide::inflate::decompress_to_vec_with_limit(payload, raw_len)
                .or_else(|e| bad(format!("deflate block: {e:?}")))?;
            if s.len() != raw_len {
                return bad("deflate block has the wrong length");
            }
            let mut r = vec![0u8; raw_len];
            for i in 0..count {
                for j in 0..w {
                    r[i * w + j] = s[j * count + i];
                }
            }
            Cow::Owned(r)
        }
    };
    if raw.len() != raw_len {
        return bad("data block has the wrong length");
    }
    Ok(match p {
        Precision::F64 => raw.chunks_exact(8).map(|b| f64::from_bits(le64(b))).collect(),
        Precision::F32 => raw
            .chunks_exact(4)
            .map(|b| f64::from(f32::from_bits(u32::from_le_bytes(b.try_into().expect("4 bytes")))))
            .collect(),
    })
}

// ---------------------------------------------------------------- writer

/// Streaming writer: the solver output path opens it once, then adds snapshots field by field.
pub struct Writer {
    dir: PathBuf,
    out: BufWriter<File>,
    pos: u64,
    opts: WriteOptions,
    mesh: MeshPrefix,
    snaps: Vec<SnapInfo>,
}

impl Writer {
    /// Create `dir/store.rts` with the mesh. Refuses to replace an existing store.
    pub fn create(dir: &Path, mesh: &Mesh, opts: WriteOptions) -> Result<Self> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&mesh.content_hash().to_le_bytes());
        payload.extend_from_slice(&(mesh.n_cells() as u64).to_le_bytes());
        payload.extend_from_slice(&(mesh.n_points() as u64).to_le_bytes());
        payload.extend_from_slice(&rcfd_io::container::mesh_to_bytes(mesh));
        Self::create_with_mesh_payload(dir, payload, opts)
    }

    fn create_with_mesh_payload(dir: &Path, payload: Vec<u8>, opts: WriteOptions) -> Result<Self> {
        if opts.block_values == 0 || opts.block_values > MAX_BLOCK_VALUES {
            return bad(format!("block_values must lie in 1..={MAX_BLOCK_VALUES}"));
        }
        std::fs::create_dir_all(dir)?;
        let path = dir.join(STORE_FILE);
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .or_else(|e| bad(format!("cannot create {}: {e}", path.display())))?;
        let mut w = Writer {
            dir: dir.to_path_buf(),
            out: BufWriter::new(file),
            pos: 0,
            opts,
            mesh: MeshPrefix {
                end: HEADER_LEN + FRAME_HEAD_LEN + payload.len() as u64,
                hash: le64(&payload),
                n_cells: le64(&payload[8..]) as usize,
                n_points: le64(&payload[16..]) as usize,
            },
            snaps: Vec::new(),
        };
        w.put(&header_bytes())?;
        w.put_frame(K_MESH, &payload)?;
        w.flush()?;
        Ok(w)
    }

    /// Reopen an existing store to add snapshots (a continued run, or after a crash): the index is rebuilt by
    /// scanning and everything after the last complete snapshot (an interrupted snapshot, the old footer) is cut.
    pub fn open_append(dir: &Path, opts: WriteOptions) -> Result<Self> {
        if opts.block_values == 0 || opts.block_values > MAX_BLOCK_VALUES {
            return bad(format!("block_values must lie in 1..={MAX_BLOCK_VALUES}"));
        }
        let path = dir.join(STORE_FILE);
        let mut file = OpenOptions::new().read(true).write(true).open(&path)?;
        let len = file.metadata()?.len();
        check_header(&mut file, len)?;
        let mesh = mesh_prefix(&mut file, len)?;
        let s = scan(&mut file, len, &mesh, mesh.end, None)?;
        file.set_len(s.committed_end)?;
        file.seek(SeekFrom::Start(s.committed_end))?;
        Ok(Writer {
            dir: dir.to_path_buf(),
            out: BufWriter::new(file),
            pos: s.committed_end,
            opts,
            mesh,
            snaps: s.snaps,
        })
    }

    /// Committed snapshots.
    pub fn snapshots(&self) -> &[SnapInfo] {
        &self.snaps
    }

    /// Content hash of the stored mesh.
    pub fn mesh_hash(&self) -> u64 {
        self.mesh.hash
    }

    /// Start a snapshot. `step` must exceed the previous snapshot's step and `time` must not be smaller than its time.
    pub fn snapshot(&mut self, step: u64, time: f64) -> Result<SnapshotWriter<'_>> {
        if !time.is_finite() {
            return bad("snapshot time must be finite");
        }
        if !monotone(self.snaps.last(), step, time) {
            return bad(format!(
                "snapshot (step {step}, time {time}) must come after step {} at time {}",
                self.snaps.last().map_or(0, |s| s.step),
                self.snaps.last().map_or(0.0, |s| s.time)
            ));
        }
        Ok(SnapshotWriter {
            w: self,
            dir: SnapDir {
                step,
                time,
                fields: Vec::new(),
            },
        })
    }

    /// Hand buffered bytes to the operating system (done at every commit; no fsync).
    pub fn flush(&mut self) -> Result<()> {
        self.out.flush()?;
        Ok(())
    }

    /// Write the footer index and trailer, then the advisory `index.json`.
    pub fn finish(mut self) -> Result<()> {
        let foot = Footer {
            mesh: HEADER_LEN,
            snapshots: self.snaps.iter().map(|s| (s.step, s.time, s.offset, s.end)).collect(),
        };
        let off = self.pos;
        self.put_frame(K_FOOT, &serde_json::to_vec(&foot).expect("footer serializable"))?;
        let mut t = [0u8; 20];
        t[..8].copy_from_slice(&off.to_le_bytes());
        t[8..12].copy_from_slice(&TRAILER_MAGIC);
        let s = fnv64(&t[..12]);
        t[12..].copy_from_slice(&s.to_le_bytes());
        self.put(&t)?;
        self.flush()?;
        // the store is authoritative; a failed mirror (e.g. a reader holding index.json open on Windows) is not an error
        let _ = write_index(&self.dir, &self.mesh, &self.snaps);
        Ok(())
    }

    fn put(&mut self, b: &[u8]) -> Result<()> {
        self.out.write_all(b)?;
        self.pos += b.len() as u64;
        Ok(())
    }

    fn put_frame(&mut self, kind: [u8; 4], payload: &[u8]) -> Result<u64> {
        let off = self.pos;
        self.put(&head_bytes(kind, payload))?;
        self.put(payload)?;
        Ok(off)
    }
}

fn write_index(dir: &Path, m: &MeshPrefix, snaps: &[SnapInfo]) -> Result<()> {
    let v = serde_json::json!({
        "format": "rcfd-tsstore-index", "version": VERSION, "store": STORE_FILE,
        "mesh_hash": format!("{:016x}", m.hash), "n_cells": m.n_cells, "n_points": m.n_points,
        "snapshots": snaps.iter().map(|s| serde_json::json!({"step": s.step, "time": s.time})).collect::<Vec<_>>(),
    });
    let tmp = dir.join("index.json.tmp");
    std::fs::write(&tmp, v.to_string())?;
    std::fs::rename(&tmp, dir.join(INDEX_FILE))?;
    Ok(())
}

/// One snapshot being written; nothing is visible to readers until [`SnapshotWriter::commit`].
pub struct SnapshotWriter<'a> {
    w: &'a mut Writer,
    dir: SnapDir,
}

impl SnapshotWriter<'_> {
    /// Add a field: `values.len()` must equal the mesh's cell count (`Location::Cell`) or point count. With
    /// [`Precision::F32`] finite values beyond the single-precision range are refused.
    pub fn field(
        &mut self,
        name: &str,
        unit: &str,
        location: Location,
        values: &[f64],
        precision: Precision,
    ) -> Result<()> {
        if name.is_empty() || name.len() > 255 || unit.len() > 255 {
            return bad("field names must have 1-255 bytes and units at most 255");
        }
        if self.dir.fields.iter().any(|f| f.name == name) {
            return bad(format!("field '{name}' given twice in one snapshot"));
        }
        let want = match location {
            Location::Cell => self.w.mesh.n_cells,
            Location::Point => self.w.mesh.n_points,
        };
        if values.len() != want {
            return bad(format!(
                "field '{name}' has {} values, the mesh has {want}",
                values.len()
            ));
        }
        if precision == Precision::F32 && values.iter().any(|x| x.is_finite() && (*x as f32).is_infinite()) {
            return bad(format!(
                "field '{name}' has values beyond the single-precision range; store it as f64"
            ));
        }
        let (bv, codec) = (self.w.opts.block_values, self.w.opts.codec);
        let mut blocks = Vec::with_capacity(values.len().div_ceil(bv));
        for chunk in values.chunks(bv) {
            blocks.push(self.w.put_frame(K_DATA, &encode(chunk, precision, codec))?);
        }
        self.dir.fields.push(FieldInfo {
            name: name.into(),
            unit: unit.into(),
            location,
            n: values.len(),
            precision,
            codec,
            block_values: bv,
            blocks,
        });
        Ok(())
    }

    /// Hand the blocks written so far to the operating system (they stay invisible until the commit).
    pub fn flush(&mut self) -> Result<()> {
        self.w.flush()
    }

    /// Make the snapshot visible (writes its directory frame and flushes).
    pub fn commit(self) -> Result<SnapInfo> {
        let json = serde_json::to_vec(&self.dir).expect("snapshot directory serializable");
        let offset = self.w.put_frame(K_SNAP, &json)?;
        self.w.flush()?;
        let s = SnapInfo {
            step: self.dir.step,
            time: self.dir.time,
            offset,
            end: self.w.pos,
        };
        self.w.snaps.push(s);
        Ok(s)
    }
}

// ---------------------------------------------------------------- reader

/// How a snapshot is chosen for a requested step or time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// The closest one (the earlier one on a tie).
    Nearest,
    /// The last one at or before the request.
    Floor,
}

/// How a field is sampled at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The snapshot chosen by [`Pick`].
    Pick(Pick),
    /// Linear in time between the two snapshots around the time (flagged as interpolated); the stored values
    /// exactly when the time is a snapshot time.
    Linear,
}

/// A field sampled at a time.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// Values.
    pub values: Vec<f64>,
    /// True when the values are an interpolation between two snapshots, not stored values.
    pub interpolated: bool,
    /// Snapshot index used (the earlier one when interpolated).
    pub lower: usize,
    /// Later snapshot index (equal to `lower` when not interpolated).
    pub upper: usize,
    /// Weight of `upper` (0 when not interpolated).
    pub weight: f64,
}

/// Random-access reader (seek-based; holds no field data between calls).
pub struct Reader {
    file: File,
    len: u64,
    mesh: MeshPrefix,
    snaps: Vec<SnapInfo>,
    source: IndexSource,
}

impl Reader {
    /// Open `dir/store.rts`: the footer index when it is valid, otherwise a scan.
    pub fn open(dir: &Path) -> Result<Self> {
        let mut file = File::open(dir.join(STORE_FILE))?;
        let len = file.metadata()?.len();
        check_header(&mut file, len)?;
        let mesh = mesh_prefix(&mut file, len)?;
        let (snaps, source) = match read_footer(&mut file, len, &mesh) {
            Ok(s) => (s, IndexSource::Footer),
            Err(reason) => {
                let s = scan(&mut file, len, &mesh, mesh.end, None)?;
                let tail_bytes = len - s.committed_end;
                (s.snaps, IndexSource::Scanned { reason, tail_bytes })
            }
        };
        Ok(Reader {
            file,
            len,
            mesh,
            snaps,
            source,
        })
    }

    /// Pick up snapshots a live writer committed since the last open/refresh (scanned index only); returns how
    /// many were added.
    pub fn refresh(&mut self) -> Result<usize> {
        let IndexSource::Scanned { reason, .. } = &self.source else {
            return Ok(0);
        };
        let reason = reason.clone();
        let len = self.file.metadata()?.len();
        let last = self.snaps.last().copied();
        let start = last.map_or(self.mesh.end, |s| s.end);
        let s = scan(&mut self.file, len, &self.mesh, start, last)?;
        let added = s.snaps.len();
        self.snaps.extend(s.snaps);
        self.len = len;
        self.source = IndexSource::Scanned {
            reason,
            tail_bytes: len - s.committed_end,
        };
        Ok(added)
    }

    /// Where the index came from.
    pub fn index_source(&self) -> &IndexSource {
        &self.source
    }

    /// Committed snapshots in order.
    pub fn snapshots(&self) -> &[SnapInfo] {
        &self.snaps
    }

    /// Cells of the stored mesh.
    pub fn n_cells(&self) -> usize {
        self.mesh.n_cells
    }

    /// Points of the stored mesh.
    pub fn n_points(&self) -> usize {
        self.mesh.n_points
    }

    /// Content hash of the stored mesh as recorded by the writer.
    pub fn mesh_hash(&self) -> u64 {
        self.mesh.hash
    }

    /// Read, verify and decode the mesh; its content hash must equal the recorded one.
    pub fn mesh(&mut self) -> Result<Mesh> {
        let p = self.mesh_payload()?;
        let mesh = rcfd_io::container::mesh_from_bytes(&p[24..])?;
        if mesh.content_hash() != self.mesh.hash
            || mesh.n_cells() != self.mesh.n_cells
            || mesh.n_points() != self.mesh.n_points
        {
            return bad("stored mesh does not match its recorded hash and sizes");
        }
        Ok(mesh)
    }

    fn mesh_payload(&mut self) -> Result<Vec<u8>> {
        let p = payload_at(&mut self.file, HEADER_LEN, self.len, K_MESH, u64::MAX)?;
        if le64(&p) != self.mesh.hash
            || le64(&p[8..]) as usize != self.mesh.n_cells
            || le64(&p[16..]) as usize != self.mesh.n_points
        {
            return bad("mesh frame changed while open");
        }
        Ok(p)
    }

    fn dir(&mut self, i: usize) -> Result<SnapDir> {
        let Some(s) = self.snaps.get(i).copied() else {
            return bad(format!("no snapshot {i} (the store has {})", self.snaps.len()));
        };
        let p = payload_at(&mut self.file, s.offset, self.len, K_SNAP, MAX_JSON_FRAME)?;
        if s.offset + FRAME_HEAD_LEN + p.len() as u64 != s.end {
            return bad(format!("snapshot {i} does not match the index"));
        }
        let d: SnapDir = serde_json::from_slice(&p).or_else(|e| bad(format!("snapshot {i}: {e}")))?;
        check_dir(&d, &self.mesh, s.offset)?;
        if d.step != s.step || d.time.to_bits() != s.time.to_bits() {
            return bad(format!("snapshot {i} does not match the index"));
        }
        Ok(d)
    }

    /// Fields of snapshot `i`.
    pub fn fields(&mut self, i: usize) -> Result<Vec<FieldInfo>> {
        Ok(self.dir(i)?.fields)
    }

    fn info(&mut self, i: usize, name: &str) -> Result<FieldInfo> {
        let d = self.dir(i)?;
        match d.fields.into_iter().find(|f| f.name == name) {
            Some(f) => Ok(f),
            None => bad(format!("snapshot {i} (step {}) has no field '{name}'", d.step)),
        }
    }

    fn block(&mut self, f: &FieldInfo, b: usize) -> Result<Vec<f64>> {
        let count = f.block_values.min(f.n - b * f.block_values);
        let max = (count as u64) * 16 + 1024;
        let p = payload_at(&mut self.file, f.blocks[b], self.len, K_DATA, max)?;
        decode(&p, count, f.precision, f.codec)
    }

    /// All values of field `name` of snapshot `i`.
    pub fn read_field(&mut self, i: usize, name: &str) -> Result<Vec<f64>> {
        let f = self.info(i, name)?;
        let mut out = Vec::with_capacity(f.n);
        for b in 0..f.blocks.len() {
            out.extend(self.block(&f, b)?);
        }
        Ok(out)
    }

    /// Values of field `name` of snapshot `i` at the given cell (or point) indices, reading only the blocks
    /// that hold them.
    pub fn read_subset(&mut self, i: usize, name: &str, idx: &[usize]) -> Result<Vec<f64>> {
        let f = self.info(i, name)?;
        if let Some(&k) = idx.iter().find(|&&k| k >= f.n) {
            return bad(format!("index {k} outside field '{name}' of {} values", f.n));
        }
        let mut order: Vec<usize> = (0..idx.len()).collect();
        order.sort_by_key(|&j| idx[j]);
        let mut out = vec![0.0; idx.len()];
        let mut cur: Option<(usize, Vec<f64>)> = None;
        for j in order {
            let b = idx[j] / f.block_values;
            if cur.as_ref().map_or(true, |c| c.0 != b) {
                cur = Some((b, self.block(&f, b)?));
            }
            let c = cur.as_ref().expect("block loaded");
            out[j] = c.1[idx[j] - b * f.block_values];
        }
        Ok(out)
    }

    /// Snapshot for a step.
    pub fn by_step(&self, step: u64, pick: Pick) -> Option<usize> {
        let k = self.snaps.partition_point(|s| s.step <= step);
        self.choose(k, pick, |s| s.step as f64, step as f64)
    }

    /// Snapshot for a time (equal times: the latest step at that time counts as the floor).
    pub fn by_time(&self, t: f64, pick: Pick) -> Option<usize> {
        if t.is_nan() {
            return None;
        }
        let k = self.snaps.partition_point(|s| s.time <= t);
        self.choose(k, pick, |s| s.time, t)
    }

    fn choose(&self, k: usize, pick: Pick, key: impl Fn(&SnapInfo) -> f64, x: f64) -> Option<usize> {
        let floor = k.checked_sub(1);
        match pick {
            Pick::Floor => floor,
            Pick::Nearest => match (floor, self.snaps.get(k)) {
                (Some(f), Some(n)) => Some(if key(n) - x < x - key(&self.snaps[f]) { k } else { f }),
                (Some(f), None) => Some(f),
                (None, Some(_)) => Some(k),
                (None, None) => None,
            },
        }
    }

    /// Field `name` at time `t`. [`Mode::Linear`] needs `t` within the stored times.
    pub fn field_at_time(&mut self, name: &str, t: f64, mode: Mode) -> Result<Sample> {
        let exact = |values: Vec<f64>, i: usize| Sample {
            values,
            interpolated: false,
            lower: i,
            upper: i,
            weight: 0.0,
        };
        match mode {
            Mode::Pick(p) => {
                let Some(i) = self.by_time(t, p) else {
                    return bad(format!("no snapshot for time {t}"));
                };
                Ok(exact(self.read_field(i, name)?, i))
            }
            Mode::Linear => {
                let (first, last) = match (self.snaps.first(), self.snaps.last()) {
                    (Some(a), Some(b)) => (a.time, b.time),
                    _ => return bad("the store has no snapshots"),
                };
                if !(t >= first && t <= last) {
                    return bad(format!("time {t} outside the stored range [{first}, {last}]"));
                }
                let i0 = self.by_time(t, Pick::Floor).expect("t >= first time");
                let t0 = self.snaps[i0].time;
                if t == t0 {
                    return Ok(exact(self.read_field(i0, name)?, i0));
                }
                let i1 = i0 + 1; // t0 < t <= last, so a later snapshot with a larger time exists
                let t1 = self.snaps[i1].time;
                let w = (t - t0) / (t1 - t0);
                let a = self.read_field(i0, name)?;
                let b = self.read_field(i1, name)?;
                let values = a.iter().zip(&b).map(|(x, y)| x + w * (y - x)).collect();
                Ok(Sample {
                    values,
                    interpolated: true,
                    lower: i0,
                    upper: i1,
                    weight: w,
                })
            }
        }
    }

    /// Read and check every checksum: mesh, every snapshot directory and every block.
    pub fn verify(&mut self) -> Result<()> {
        self.mesh()?;
        for i in 0..self.snaps.len() {
            for f in self.dir(i)?.fields {
                for b in 0..f.blocks.len() {
                    self.block(&f, b)?;
                }
            }
        }
        Ok(())
    }

    fn raw_block(&mut self, f: &FieldInfo, b: usize) -> Result<Vec<u8>> {
        let count = f.block_values.min(f.n - b * f.block_values);
        let p = payload_at(
            &mut self.file,
            f.blocks[b],
            self.len,
            K_DATA,
            (count as u64) * 16 + 1024,
        )?;
        decode(&p, count, f.precision, f.codec)?; // a copied block must decode
        Ok(p)
    }
}

/// Retention: copy the store in `src` to a new store in `dst` keeping snapshots 0, k, 2k, ... and the last one.
/// Blocks are copied verbatim after their checksums are verified (bit-exact, codec and precision kept). Returns
/// the kept snapshots. `dst` must not hold a store yet.
pub fn retain_every(src: &Path, dst: &Path, k: usize) -> Result<Vec<SnapInfo>> {
    if k == 0 {
        return bad("retention interval must be at least 1");
    }
    let mut r = Reader::open(src)?;
    let n = r.snaps.len();
    let mut w = Writer::create_with_mesh_payload(dst, r.mesh_payload()?, WriteOptions::default())?;
    for i in (0..n).filter(|i| i % k == 0 || *i + 1 == n) {
        let d = r.dir(i)?;
        let mut fields = Vec::with_capacity(d.fields.len());
        for f in d.fields {
            let mut blocks = Vec::with_capacity(f.blocks.len());
            for b in 0..f.blocks.len() {
                let p = r.raw_block(&f, b)?;
                blocks.push(w.put_frame(K_DATA, &p)?);
            }
            fields.push(FieldInfo { blocks, ..f });
        }
        let s = w.snapshot(d.step, d.time)?;
        SnapshotWriter {
            w: s.w,
            dir: SnapDir {
                step: d.step,
                time: d.time,
                fields,
            },
        }
        .commit()?;
    }
    let kept = w.snaps.clone();
    w.finish()?;
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_deflate_round_trip_is_bit_exact() {
        let v: Vec<f64> = (0..1000)
            .map(|i| (i as f64 * 0.37).sin() * 1e-3 + 2.0)
            .chain([f64::NAN, -0.0, 1e-310])
            .collect();
        for p in [Precision::F64, Precision::F32] {
            for c in [Codec::Raw, Codec::ShuffleDeflate] {
                let back = decode(&encode(&v, p, c), v.len(), p, c).unwrap();
                let direct = decode(&encode(&v, p, Codec::Raw), v.len(), p, Codec::Raw).unwrap();
                assert!(back.iter().zip(&direct).all(|(a, b)| a.to_bits() == b.to_bits()));
            }
        }
        let back = decode(
            &encode(&v, Precision::F64, Codec::ShuffleDeflate),
            v.len(),
            Precision::F64,
            Codec::ShuffleDeflate,
        )
        .unwrap();
        assert!(back.iter().zip(&v).all(|(a, b)| a.to_bits() == b.to_bits()));
        // wrong declared length is refused
        assert!(decode(
            &encode(&v, Precision::F64, Codec::ShuffleDeflate),
            v.len() - 1,
            Precision::F64,
            Codec::ShuffleDeflate
        )
        .is_err());
    }
}
