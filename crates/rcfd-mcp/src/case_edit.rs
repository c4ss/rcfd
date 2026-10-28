//! Smallest-change edits of case files for `edit_case` (ADR 0135 amendment 1 item 5).
//!
//! A TOML case gets the `key = value` line of each dotted setting replaced in place (comments and layout kept), or
//! added to its table; a JSON case gets the value set in its parsed document. The caller re-parses the result and
//! compares it with the expected settings tree; anything this module cannot place returns `None` and the caller
//! writes the canonical form instead, so a wrong guess here can never reach the disk.

use serde_json::Value;

/// `value` as a TOML literal (`None` for `null`, which TOML cannot write).
pub(crate) fn toml_literal(value: &Value) -> Option<String> {
    Some(match value {
        Value::Null => return None,
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        // JSON string escapes are a subset of TOML basic-string escapes
        Value::String(s) => serde_json::to_string(s).ok()?,
        Value::Array(a) => {
            let items: Option<Vec<String>> = a.iter().map(toml_literal).collect();
            format!("[{}]", items?.join(", "))
        }
        Value::Object(o) => {
            let items: Option<Vec<String>> = o
                .iter()
                .map(|(k, v)| Some(format!("{} = {}", key_literal(k), toml_literal(v)?)))
                .collect();
            if o.is_empty() {
                "{}".into()
            } else {
                format!("{{ {} }}", items?.join(", "))
            }
        }
    })
}

fn bare(k: &str) -> bool {
    !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn key_literal(k: &str) -> String {
    if bare(k) {
        k.to_string()
    } else {
        serde_json::to_string(k).unwrap_or_default()
    }
}

/// A dotted key as written in the file (`a . "b"` -> `a.b`); `None` if it has a form this module does not read.
fn read_key(s: &str) -> Option<String> {
    let mut parts = Vec::new();
    for p in s.split('.') {
        let p = p.trim();
        let p = if p.len() >= 2
            && ((p.starts_with('"') && p.ends_with('"')) || (p.starts_with('\'') && p.ends_with('\'')))
        {
            &p[1..p.len() - 1]
        } else if bare(p) {
            p
        } else {
            return None;
        };
        if p.contains(['"', '\'', '\\']) {
            return None;
        }
        parts.push(p.to_string());
    }
    Some(parts.join("."))
}

/// The table a header line opens (`[a.b]` -> `a.b`); `Some(None)` for an array-of-tables header, `None` for no header.
fn header(line: &str) -> Option<Option<String>> {
    let t = line.trim();
    if t.starts_with("[[") {
        return Some(None);
    }
    let inner = t.strip_prefix('[')?;
    let end = inner.find(']')?;
    let rest = inner[end + 1..].trim();
    if !(rest.is_empty() || rest.starts_with('#')) {
        return Some(None);
    }
    Some(read_key(&inner[..end]))
}

/// Byte range of the value of `key = value` on one line, and whether the value is complete on that line.
fn value_span(line: &str) -> Option<(usize, usize)> {
    let eq = line.find('=')?;
    let b = line.as_bytes();
    let start = eq + 1 + (line[eq + 1..].len() - line[eq + 1..].trim_start().len());
    let rest = &line[start..];
    if rest.starts_with("\"\"\"") || rest.starts_with("'''") {
        return None; // multi-line string
    }
    let (mut depth, mut i) = (0i32, start);
    let mut end = b.len();
    while i < b.len() {
        match b[i] {
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                if i >= b.len() {
                    return None;
                }
            }
            b'\'' => {
                i += 1;
                while i < b.len() && b[i] != b'\'' {
                    i += 1;
                }
                if i >= b.len() {
                    return None;
                }
            }
            b'[' | b'{' => depth += 1,
            b']' | b'}' => depth -= 1,
            b'#' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 {
        return None; // a value that continues on the next line
    }
    let trimmed_end = start + line[start..end].trim_end().len();
    Some((start, trimmed_end))
}

/// Set dotted `path` to `value` in TOML `text`; `None` when the place cannot be found with confidence.
pub(crate) fn toml_set(text: &str, path: &str, value: &Value) -> Option<String> {
    let lit = toml_literal(value)?;
    let (table, key) = match path.rsplit_once('.') {
        Some((t, k)) => (t.to_string(), k.to_string()),
        None => (String::new(), path.to_string()),
    };
    if path.split('.').any(|s| s.is_empty()) {
        return None;
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let headers: Vec<(usize, Option<String>)> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| header(l).map(|h| (i, h)))
        .collect();
    let next_header = |after: usize| headers.iter().map(|h| h.0).find(|&i| i > after).unwrap_or(lines.len());
    // [first body line, end) of the target table
    let region = if table.is_empty() {
        Some((0, headers.first().map_or(lines.len(), |h| h.0)))
    } else {
        let mut found = headers.iter().filter(|h| h.1.as_deref() == Some(table.as_str()));
        match (found.next(), found.next()) {
            (Some(h), None) => Some((h.0 + 1, next_header(h.0))),
            (None, _) => None,
            (Some(_), Some(_)) => return None, // the table appears twice: not a valid file anyway
        }
    };
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    match region {
        Some((s, e)) => {
            for (i, l) in lines.iter().enumerate().take(e).skip(s) {
                let t = l.trim_start();
                if t.starts_with('#') || t.is_empty() {
                    continue;
                }
                let Some(eq) = l.find('=') else { continue };
                if read_key(&l[..eq]).as_deref() == Some(key.as_str()) {
                    let (vs, ve) = value_span(l)?;
                    out[i] = format!("{}{}{}", &l[..vs], lit, &l[ve..]);
                    return Some(out.concat());
                }
            }
            // not present: add after the table's last non-blank line
            let mut at = s;
            for (i, l) in lines.iter().enumerate().take(e).skip(s) {
                if !l.trim().is_empty() {
                    at = i + 1;
                }
            }
            let nl = if at > 0 && !lines[at - 1].ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.insert(at, format!("{nl}{} = {lit}\n", key_literal(&key)));
            Some(out.concat())
        }
        None => {
            let mut s = text.to_string();
            if !s.is_empty() && !s.ends_with('\n') {
                s.push('\n');
            }
            let head: Vec<String> = table.split('.').map(key_literal).collect();
            s.push_str(&format!("\n[{}]\n{} = {lit}\n", head.join("."), key_literal(&key)));
            Some(s)
        }
    }
}

/// Set dotted `path` to `value` in a parsed JSON case document.
pub(crate) fn json_set(doc: &mut Value, path: &str, value: &Value) -> Option<()> {
    let segs: Vec<&str> = path.split('.').collect();
    let mut cur = doc;
    for s in &segs[..segs.len() - 1] {
        let o = cur.as_object_mut()?;
        cur = o
            .entry(s.to_string())
            .or_insert_with(|| Value::Object(Default::default()));
    }
    cur.as_object_mut()?
        .insert(segs[segs.len() - 1].to_string(), value.clone());
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CASE: &str = "title = \"t\" # name\n[mesh]\nnx = 16\nny = 16\n\n[physics]\nnu = 0.01   # viscosity\n\n[boundaries.lid]\nvelocity_value = [1.0, 0.0, 0.0]\n";

    #[test]
    fn replaces_in_place_and_keeps_comments() {
        let t = toml_set(CASE, "physics.nu", &json!(0.02)).unwrap();
        assert!(t.contains("nu = 0.02   # viscosity\n"), "{t}");
        let t = toml_set(CASE, "title", &json!("a \"q\"")).unwrap();
        assert!(t.starts_with("title = \"a \\\"q\\\"\" # name\n"), "{t}");
        let t = toml_set(CASE, "boundaries.lid.velocity_value", &json!([2.0, 0.0, 0.0])).unwrap();
        assert!(t.contains("velocity_value = [2.0, 0.0, 0.0]\n"), "{t}");
    }

    #[test]
    fn adds_missing_keys_and_tables() {
        let t = toml_set(CASE, "mesh.nz", &json!(1)).unwrap();
        assert!(t.contains("ny = 16\nnz = 1\n\n[physics]"), "{t}");
        let t = toml_set(CASE, "solver.max_iterations", &json!(30)).unwrap();
        assert!(t.ends_with("\n[solver]\nmax_iterations = 30\n"), "{t}");
        let t = toml_set(CASE, "description", &json!("d")).unwrap();
        assert!(
            t.starts_with("title = \"t\" # name\ndescription = \"d\"\n[mesh]"),
            "{t}"
        );
    }

    #[test]
    fn declines_what_it_cannot_place() {
        assert!(toml_set("a = [1,\n2]\n", "a", &json!(1)).is_none());
        assert!(toml_set("a = \"\"\"x\n\"\"\"\n", "a", &json!(1)).is_none());
        assert!(toml_set(CASE, "physics.nu", &Value::Null).is_none());
    }

    #[test]
    fn json_documents() {
        let mut d = json!({"physics": {"nu": 0.01}});
        json_set(&mut d, "physics.nu", &json!(0.5)).unwrap();
        json_set(&mut d, "solver.max_iterations", &json!(3)).unwrap();
        assert_eq!(d, json!({"physics": {"nu": 0.5}, "solver": {"max_iterations": 3}}));
    }
}
