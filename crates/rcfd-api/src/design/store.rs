//! Persistent evaluation store of a design study (ADR 0067): JSON lines, one header line and one record per evaluation.
//!
//! Records are appended after each batch in request order; an incomplete last line (a write interrupted by a kill) is
//! dropped when the store is opened. A record is addressed by the exact bit pattern of its parameter vector, so a rerun of
//! the same deterministic study finds its earlier evaluations and continues where it stopped.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Value of the `format` member of the header line.
pub const FORMAT: &str = "rcfd-design-store";

/// Result of one candidate at one operating condition.
#[derive(Debug, Clone, PartialEq)]
pub struct CondResult {
    /// Condition label.
    pub label: String,
    /// Whether the run is usable (converged, all metrics finite).
    pub ok: bool,
    /// Run status (`converged`, `not_converged`, `diverged`, `failed`, `recorded`, ...).
    pub status: String,
    /// Iterations or steps spent (0 when not applicable).
    pub iterations: usize,
    /// Metric values by name (reports of the case).
    pub metrics: BTreeMap<String, f64>,
    /// Failure reason or notes.
    pub message: String,
}

impl CondResult {
    /// A successful result.
    pub fn success(label: &str, metrics: BTreeMap<String, f64>) -> Self {
        CondResult {
            label: label.to_string(),
            ok: true,
            status: "converged".into(),
            iterations: 0,
            metrics,
            message: String::new(),
        }
    }

    /// A failed result with a reason.
    pub fn failure(label: &str, status: &str, message: impl Into<String>) -> Self {
        CondResult {
            label: label.to_string(),
            ok: false,
            status: status.to_string(),
            iterations: 0,
            metrics: BTreeMap::new(),
            message: message.into(),
        }
    }

    /// JSON form.
    pub fn to_json(&self) -> Value {
        let metrics: Map<String, Value> = self
            .metrics
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    serde_json::Number::from_f64(*v).map_or(Value::Null, Value::Number),
                )
            })
            .collect();
        json!({
            "label": self.label, "ok": self.ok, "status": self.status, "iterations": self.iterations,
            "metrics": metrics, "message": self.message,
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        let metrics = v
            .get("metrics")?
            .as_object()?
            .iter()
            .map(|(k, x)| (k.clone(), x.as_f64().unwrap_or(f64::NAN)))
            .collect();
        Some(CondResult {
            label: v.get("label")?.as_str()?.to_string(),
            ok: v.get("ok")?.as_bool()?,
            status: v.get("status")?.as_str()?.to_string(),
            iterations: v.get("iterations")?.as_u64()? as usize,
            metrics,
            message: v.get("message")?.as_str()?.to_string(),
        })
    }
}

/// One evaluated candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// Order of first request (0-based), the only ordering key.
    pub index: usize,
    /// See [`key_of`].
    pub key: String,
    /// Parameter values in the order of the study definition.
    pub params: Vec<(String, f64)>,
    /// Where the evaluation came from: `evaluated` (run by rcfd) or `recorded` (metrics supplied by the caller).
    pub source: String,
    /// One result per operating condition.
    pub conditions: Vec<CondResult>,
    /// Failure reason or notes.
    pub message: String,
}

impl Record {
    /// Whether every condition produced a usable result.
    pub fn ok(&self) -> bool {
        !self.conditions.is_empty() && self.conditions.iter().all(|c| c.ok)
    }

    /// JSON form (one line of the store).
    pub fn to_json(&self) -> Value {
        let params: Map<String, Value> = self
            .params
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    serde_json::Number::from_f64(*v).map_or(Value::Null, Value::Number),
                )
            })
            .collect();
        json!({
            "index": self.index, "key": self.key, "params": params, "source": self.source,
            "conditions": self.conditions.iter().map(CondResult::to_json).collect::<Vec<_>>(),
            "message": self.message,
        })
    }

    /// Parse the JSON form (the parameters come back sorted by name; look them up by name).
    pub fn from_json(v: &Value) -> Option<Self> {
        let params: Vec<(String, f64)> = v
            .get("params")?
            .as_object()?
            .iter()
            .map(|(k, x)| Some((k.clone(), x.as_f64()?)))
            .collect::<Option<_>>()?;
        Some(Record {
            index: v.get("index")?.as_u64()? as usize,
            key: v.get("key")?.as_str()?.to_string(),
            params,
            source: v.get("source")?.as_str()?.to_string(),
            conditions: v
                .get("conditions")?
                .as_array()?
                .iter()
                .map(CondResult::from_json)
                .collect::<Option<_>>()?,
            message: v.get("message")?.as_str()?.to_string(),
        })
    }
}

/// Key of a parameter vector: the hexadecimal bit patterns joined by `-` (exact, so no tolerance is involved).
pub fn key_of(x: &[f64]) -> String {
    x.iter()
        .map(|v| format!("{:016x}", v.to_bits()))
        .collect::<Vec<_>>()
        .join("-")
}

/// The store: records in memory plus an optional file.
#[derive(Debug)]
pub struct Store {
    path: Option<PathBuf>,
    signature: String,
    records: Vec<Record>,
    by_key: HashMap<String, usize>,
}

impl Store {
    /// A store without a file.
    pub fn memory(signature: &str) -> Store {
        Store {
            path: None,
            signature: signature.to_string(),
            records: Vec::new(),
            by_key: HashMap::new(),
        }
    }

    /// Open (or prepare) the store file. A file written for another `signature` is refused. An incomplete last line is
    /// removed from the file.
    pub fn open(path: &Path, signature: &str) -> Result<Store, String> {
        Self::open_impl(path, signature, true)
    }

    /// Like [`Store::open`] but never modifies the file (an incomplete last line is ignored in memory only).
    pub fn open_read(path: &Path, signature: &str) -> Result<Store, String> {
        Self::open_impl(path, signature, false)
    }

    fn open_impl(path: &Path, signature: &str, repair: bool) -> Result<Store, String> {
        let mut st = Store {
            path: Some(path.to_path_buf()),
            signature: signature.to_string(),
            records: Vec::new(),
            by_key: HashMap::new(),
        };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(st),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        let complete = text.ends_with('\n') || text.is_empty();
        let mut lines: Vec<&str> = text.lines().collect();
        if !complete {
            lines.pop();
        }
        for (n, line) in lines.iter().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let v: Value =
                serde_json::from_str(line).map_err(|e| format!("{} line {}: not JSON: {e}", path.display(), n + 1))?;
            if n == 0 {
                if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
                    return Err(format!(
                        "{} is not a design store (missing header line)",
                        path.display()
                    ));
                }
                let sig = v.get("signature").and_then(Value::as_str).unwrap_or("");
                if sig != signature {
                    return Err(format!(
                        "{} belongs to another study definition (signature {sig}, expected {signature}); \
                         use a new study directory or delete the store",
                        path.display()
                    ));
                }
                continue;
            }
            let r = Record::from_json(&v)
                .ok_or_else(|| format!("{} line {}: not a store record", path.display(), n + 1))?;
            if r.index != st.records.len() {
                return Err(format!(
                    "{} line {}: record index {} out of order (expected {})",
                    path.display(),
                    n + 1,
                    r.index,
                    st.records.len()
                ));
            }
            st.by_key.insert(r.key.clone(), st.records.len());
            st.records.push(r);
        }
        if !complete && repair {
            // drop the partial line so that new records are appended after a complete one; truncating in place (instead of
            // rewriting the file) cannot lose the complete records if this process is killed meanwhile (ADR 0121)
            let keep = text.rfind('\n').map_or(0, |i| i + 1) as u64;
            std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .and_then(|f| f.set_len(keep))
                .map_err(|e| format!("cannot truncate {}: {e}", path.display()))?;
        }
        Ok(st)
    }

    /// All records in index order.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Record with `key`.
    pub fn get(&self, key: &str) -> Option<&Record> {
        self.by_key.get(key).map(|&i| &self.records[i])
    }

    /// Index the next new record gets.
    pub fn next_index(&self) -> usize {
        self.records.len()
    }

    /// Append records (their `index` must continue the sequence and their keys must be new).
    pub fn append(&mut self, recs: Vec<Record>) -> Result<(), String> {
        if recs.is_empty() {
            return Ok(());
        }
        for (k, r) in recs.iter().enumerate() {
            if r.index != self.records.len() + k {
                return Err(format!("record index {} does not continue the store", r.index));
            }
        }
        if let Some(p) = &self.path {
            let fresh = !p.exists();
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
            }
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .map_err(|e| format!("cannot open {}: {e}", p.display()))?;
            let mut text = String::new();
            if fresh {
                text.push_str(&json!({"format": FORMAT, "version": 1, "signature": self.signature}).to_string());
                text.push('\n');
            }
            for r in &recs {
                text.push_str(&r.to_json().to_string());
                text.push('\n');
            }
            f.write_all(text.as_bytes())
                .and_then(|()| f.flush())
                .map_err(|e| format!("cannot write {}: {e}", p.display()))?;
        }
        for r in recs {
            self.by_key.insert(r.key.clone(), self.records.len());
            self.records.push(r);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(i: usize, x: f64) -> Record {
        let mut m = BTreeMap::new();
        m.insert("f".to_string(), x * x);
        Record {
            index: i,
            key: key_of(&[x]),
            params: vec![("x".into(), x)],
            source: "evaluated".into(),
            conditions: vec![CondResult::success("base", m)],
            message: String::new(),
        }
    }

    fn tmpfile(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rcfd_store_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("evaluations.jsonl")
    }

    #[test]
    fn round_trip_is_exact_and_partial_last_line_is_dropped() {
        let p = tmpfile("rt");
        let mut s = Store::open(&p, "sig").unwrap();
        let xs = [0.1, 1.0 / 3.0, 2.0f64.sqrt(), -1e-300, 123456.789];
        s.append(xs.iter().enumerate().map(|(i, &x)| rec(i, x)).collect())
            .unwrap();
        let s2 = Store::open(&p, "sig").unwrap();
        assert_eq!(s2.records(), s.records());
        for (i, &x) in xs.iter().enumerate() {
            assert_eq!(s2.get(&key_of(&[x])).unwrap().index, i);
        }
        // truncate in the middle of the last line
        let text = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, &text[..text.len() - 20]).unwrap();
        let s3 = Store::open(&p, "sig").unwrap();
        assert_eq!(s3.records().len(), 4);
        let mut s3 = s3;
        s3.append(vec![rec(4, xs[4])]).unwrap();
        assert_eq!(Store::open(&p, "sig").unwrap().records(), s.records());
    }

    #[test]
    fn foreign_signature_and_garbage_are_refused() {
        let p = tmpfile("sig");
        Store::open(&p, "a").unwrap().append(vec![rec(0, 1.0)]).unwrap();
        assert!(Store::open(&p, "b").unwrap_err().contains("another study definition"));
        std::fs::write(&p, "not json\n").unwrap();
        assert!(Store::open(&p, "a").unwrap_err().contains("line 1"));
    }
}
