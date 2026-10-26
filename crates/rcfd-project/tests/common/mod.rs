#![allow(dead_code)]
//! Shared helpers of the ADR 0122 checks.

use std::path::PathBuf;

/// A fresh empty directory under the target temp area.
pub fn scratch(name: &str) -> PathBuf {
    let base = std::env::var_os("RCFD_PROJECT_SCRATCH")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join(format!("rcfd-project-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Self-validating payload: `gen=<g> len=<n> fnv=<hash>\n` then `n` body bytes.
pub fn payload(gen: u64, len: usize) -> Vec<u8> {
    let body: Vec<u8> = (0..len)
        .map(|i| (i as u64).wrapping_mul(31).wrapping_add(gen) as u8)
        .collect();
    let mut out = format!("gen={gen} len={len} fnv={:016x}\n", rcfd_core::hash::fnv64(&body)).into_bytes();
    out.extend_from_slice(&body);
    out
}

/// True if `bytes` is one complete payload.
pub fn payload_ok(bytes: &[u8]) -> bool {
    let Some(nl) = bytes.iter().position(|&b| b == b'\n') else {
        return false;
    };
    let Ok(head) = std::str::from_utf8(&bytes[..nl]) else {
        return false;
    };
    let mut len = None;
    let mut hash = None;
    for kv in head.split(' ') {
        match kv.split_once('=') {
            Some(("len", v)) => len = v.parse::<usize>().ok(),
            Some(("fnv", v)) => hash = u64::from_str_radix(v, 16).ok(),
            _ => {}
        }
    }
    let body = &bytes[nl + 1..];
    Some(body.len()) == len && Some(rcfd_core::hash::fnv64(body)) == hash
}
