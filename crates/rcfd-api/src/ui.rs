//! UI metadata of the case schema and help from the engine (F-826, F-830, ADR 0730).
//!
//! * The metadata (a label, one line of help, an "advanced" flag, a display unit for counts, a recommended range with its
//!   source, the docs heading that explains a key) lives in the data file `schema_ui.toml` beside the schema code, keyed by
//!   dotted schema path (`*` for a named entry). It is read with `include_str!` and laid onto the JSON Schema export by
//!   [`case_schema_ui`]; `schema.rs`, validation, parsing, unit conversion and hashing never see it.
//! * Help that is not written falls back to the first sentence of the docs section the key names (`see`), else to the
//!   first sentence of the key's own schema text, when that sentence is one line of at most [`HELP_MAX`] characters. The
//!   export says which (`x-rcfd-help-from`: `docs` or `schema`; absent for written help).
//! * The docs the help comes from are built into the binary ([`DOCS`]); `help.glossary` parses `docs/glossary.md`.
//!
//! Everything here is opt-in: `rcfd serve --ui` ([`crate::service::Service::enable_ui`]) and `rcfd schema --ui`. Without
//! the switch every reply is the one of protocol 1.1, byte for byte (ADR 0730 T1, T2).

use crate::schema::case_schema;
use crate::service::{code, Rpc, RpcError};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Identifier of the UI metadata and the `help.*` methods, reported by `service.info` (`ui`) when they are on.
pub const UI_PROTOCOL: &str = "rcfd-ui/1";
/// The `help.*` methods, appended to `methods` of `service.info` when the UI switch is on.
pub const UI_METHODS: [&str; 2] = ["help.glossary", "help.errors"];
/// Longest help line, in characters.
pub const HELP_MAX: usize = 160;

/// The metadata data file (ADR 0730 Q1).
pub const METADATA_TOML: &str = include_str!("schema_ui.toml");
/// `docs/glossary.md`, built in (ADR 0730 Q6).
pub const GLOSSARY_MD: &str = include_str!("../../../docs/glossary.md");
/// `docs/case-schema.md`, built in for `x-rcfd-see` and the help fallback.
pub const CASE_SCHEMA_MD: &str = include_str!("../../../docs/case-schema.md");
/// The docs pages `x-rcfd-see` may name, by file name under `docs/`.
pub const DOCS: [(&str, &str); 2] = [("glossary.md", GLOSSARY_MD), ("case-schema.md", CASE_SCHEMA_MD)];

/// A recommended range: advice only, never refused, and only with its source.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recommended {
    /// Lower end of the usual working range.
    pub minimum: Option<f64>,
    /// Upper end of the usual working range.
    pub maximum: Option<f64>,
    /// Public literature (author, title, year) or a validation ADR of this repository.
    pub source: String,
}

/// The metadata of one schema path.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyMeta {
    /// A short human name (`x-rcfd-label`).
    pub label: Option<String>,
    /// One written line of help (`x-rcfd-help`).
    pub help: Option<String>,
    /// The reason the key is advanced (`x-rcfd-advanced: true`); absent means common.
    pub advanced: Option<String>,
    /// A display unit for a node with neither `x-rcfd-unit` nor an SI dimension (`x-rcfd-unit`).
    pub unit: Option<String>,
    /// The docs page and heading slug that explain the key (`x-rcfd-see`), e.g. `case-schema.md#turbulence`.
    pub see: Option<String>,
    /// A usual working range with its source (`x-rcfd-recommended`).
    pub recommended: Option<Recommended>,
}

/// The state of the advanced list.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdvancedList {
    /// `proposed` until the maintainer approves it, then `approved`.
    pub status: String,
    /// Who proposed or approved it, and when.
    pub note: String,
}

/// The parsed data file.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    /// The advanced list's status.
    pub advanced_list: AdvancedList,
    /// Metadata by dotted schema path.
    pub keys: BTreeMap<String, KeyMeta>,
}

/// Parse a metadata file.
pub fn parse_metadata(text: &str) -> Result<Metadata, String> {
    toml::from_str(text).map_err(|e| format!("schema_ui.toml: {e}"))
}

/// The built-in metadata (parsed once).
pub fn metadata() -> Result<&'static Metadata, String> {
    static M: OnceLock<Result<Metadata, String>> = OnceLock::new();
    M.get_or_init(|| parse_metadata(METADATA_TOML))
        .as_ref()
        .map_err(Clone::clone)
}

// ---- docs

/// The anchor of a Markdown heading as GitHub writes it: lower case, punctuation other than `-` and `_` dropped, spaces
/// to `-`.
pub fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// The headings of a Markdown page outside code fences: `(level, text, line index)`.
fn headings(md: &str) -> Vec<(usize, &str, usize)> {
    let mut out = Vec::new();
    let mut fence = false;
    for (i, line) in md.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let level = line.chars().take_while(|&c| c == '#').count();
        if (1..=6).contains(&level) && line[level..].starts_with(' ') {
            out.push((level, line[level..].trim(), i));
        }
    }
    out
}

/// The text of the section `page#anchor` of a built-in docs page (from its heading to the next heading), or None.
pub fn doc_section(see: &str) -> Option<String> {
    let (page, anchor) = see.split_once('#')?;
    let md = DOCS.iter().find(|(p, _)| *p == page)?.1;
    let hs = headings(md);
    let k = hs.iter().position(|(_, t, _)| slug(t) == anchor)?;
    let start = hs[k].2 + 1;
    let end = hs.get(k + 1).map_or(usize::MAX, |h| h.2);
    Some(
        md.lines()
            .enumerate()
            .filter(|(i, _)| *i >= start && *i < end)
            .map(|(_, l)| l)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Markdown inline marks dropped: backticks, emphasis stars, links to their text.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after
            .find("](")
            .and_then(|j| after[j + 2..].find(')').map(|k| (j, j + 2 + k)))
        {
            Some((j, k)) if !after[..j].contains('[') => {
                out.push_str(&after[..j]);
                rest = &after[k + 1..];
            }
            _ => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace(['`', '*'], "")
}

const ABBREVIATIONS: [&str; 8] = ["e.g", "i.e", "etc", "vs", "cf", "approx", "no", "eq"];

/// The first sentence of a text (its first paragraph, Markdown marks dropped), when it is one line of at most
/// [`HELP_MAX`] characters.
pub fn first_sentence(text: &str) -> Option<String> {
    let para: Vec<&str> = text
        .lines()
        .map(str::trim)
        .skip_while(|l| l.is_empty())
        .take_while(|l| !l.is_empty())
        .collect();
    let first = para.first()?;
    // a table, a list, a code block or a quotation is not a sentence to quote
    if ["|", "-", "*", "```", ">", "    "].iter().any(|m| first.starts_with(m)) {
        return None;
    }
    let joined = plain(&para.join(" "));
    let chars: Vec<char> = joined.chars().collect();
    let mut end = chars.len();
    for i in 0..chars.len() {
        if chars[i] != '.' {
            continue;
        }
        let next = chars.get(i + 1);
        if next.is_some_and(|c| *c != ' ') {
            continue;
        }
        let word: String = chars[..i]
            .iter()
            .rev()
            .take_while(|c| !c.is_whitespace() && **c != '(')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if ABBREVIATIONS.contains(&word.to_lowercase().as_str()) {
            continue;
        }
        if next.is_none() || chars.get(i + 2).is_some_and(|c| c.is_uppercase() || !c.is_alphabetic()) {
            end = i + 1;
            break;
        }
    }
    let s: String = chars[..end].iter().collect::<String>().trim().to_string();
    (!s.is_empty() && s.chars().count() <= HELP_MAX).then_some(s)
}

// ---- the export

fn join(at: &str, name: &str) -> String {
    if at.is_empty() {
        name.to_string()
    } else {
        format!("{at}.{name}")
    }
}

/// True when a node (or the items of a list) already carries a display unit or an SI dimension.
pub fn has_unit(node: &Map<String, Value>) -> bool {
    let own = |m: &Map<String, Value>| m.contains_key("x-rcfd-unit") || m.contains_key("x-rcfd-si-dimension");
    own(node) || node.get("items").and_then(Value::as_object).is_some_and(own)
}

fn apply_node(node: &mut Map<String, Value>, path: &str, meta: &Metadata) {
    let m = meta.keys.get(path);
    if let Some(m) = m {
        if let Some(l) = &m.label {
            node.insert("x-rcfd-label".into(), json!(l));
        }
        if m.advanced.is_some() {
            node.insert("x-rcfd-advanced".into(), json!(true));
        }
        if let Some(r) = &m.recommended {
            let mut v = Map::new();
            if let Some(x) = r.minimum {
                v.insert("minimum".into(), json!(x));
            }
            if let Some(x) = r.maximum {
                v.insert("maximum".into(), json!(x));
            }
            v.insert("source".into(), json!(r.source));
            node.insert("x-rcfd-recommended".into(), Value::Object(v));
        }
        if let Some(s) = &m.see {
            node.insert("x-rcfd-see".into(), json!(s));
        }
        if let Some(u) = &m.unit {
            if !has_unit(node) {
                node.insert("x-rcfd-unit".into(), json!(u));
            }
        }
    }
    let written = m.and_then(|m| m.help.as_ref());
    if let Some(h) = written {
        node.insert("x-rcfd-help".into(), json!(h));
    } else {
        let from_docs = m
            .and_then(|m| m.see.as_deref())
            .and_then(doc_section)
            .and_then(|t| first_sentence(&t));
        let fallback = match from_docs {
            Some(h) => Some((h, "docs")),
            None => node
                .get("description")
                .and_then(Value::as_str)
                .and_then(first_sentence)
                .map(|h| (h, "schema")),
        };
        if let Some((h, from)) = fallback {
            node.insert("x-rcfd-help".into(), json!(h));
            node.insert("x-rcfd-help-from".into(), json!(from));
        }
    }
    if let Some(Value::Object(props)) = node.get_mut("properties") {
        for (k, child) in props.iter_mut() {
            if let Value::Object(c) = child {
                apply_node(c, &join(path, k), meta);
            }
        }
    }
    if node.get("x-rcfd-named-objects") == Some(&json!(true)) {
        if let Some(Value::Object(entry)) = node.get_mut("additionalProperties") {
            apply_node(entry, &join(path, "*"), meta);
        }
    }
}

/// Lay `meta` onto a case schema export (`Schema::to_json_schema`); the root gets `x-rcfd-ui`.
pub fn apply(schema: &mut Value, meta: &Metadata) {
    if let Value::Object(root) = schema {
        root.insert("x-rcfd-ui".into(), json!(UI_PROTOCOL));
        let props = root.get_mut("properties");
        if let Some(Value::Object(props)) = props {
            for (k, child) in props.iter_mut() {
                if let Value::Object(c) = child {
                    apply_node(c, k, meta);
                }
            }
        }
    }
}

/// The case schema export with the UI metadata (`settings.schema {ui: true}`, `rcfd schema --ui`).
pub fn case_schema_ui() -> Result<Value, String> {
    let meta = metadata()?;
    let mut v = case_schema().to_json_schema();
    apply(&mut v, meta);
    Ok(v)
}

/// The node of a dotted schema path (`*` for a named entry) in a JSON Schema export.
pub fn node_at<'a>(schema: &'a Value, path: &str) -> Option<&'a Value> {
    let mut n = schema;
    for seg in path.split('.') {
        n = match n.get("properties").and_then(|p| p.get(seg)) {
            Some(c) => c,
            None if seg == "*" && n.get("x-rcfd-named-objects") == Some(&json!(true)) => {
                n.get("additionalProperties")?
            }
            None => return None,
        };
    }
    Some(n)
}

/// Every dotted path of an export that a metadata entry may name (objects, collections and leaves; not list items or
/// command arguments), in the export's order.
pub fn schema_paths(schema: &Value) -> Vec<String> {
    fn walk(n: &Value, at: &str, out: &mut Vec<String>) {
        if !at.is_empty() {
            out.push(at.to_string());
        }
        if let Some(Value::Object(p)) = n.get("properties") {
            for (k, c) in p {
                walk(c, &join(at, k), out);
            }
        }
        if n.get("x-rcfd-named-objects") == Some(&json!(true)) {
            if let Some(e) = n.get("additionalProperties") {
                walk(e, &join(at, "*"), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(schema, "", &mut out);
    out
}

/// How the help of an export's nodes was found.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HelpCoverage {
    /// Nodes (objects, collections and leaves).
    pub nodes: usize,
    /// Leaves among them.
    pub leaves: usize,
    /// Nodes with written help.
    pub written: usize,
    /// Leaves with written help.
    pub written_leaves: usize,
    /// Nodes whose help is the first sentence of their docs section.
    pub docs: usize,
    /// Nodes whose help is the first sentence of their schema text.
    pub schema: usize,
    /// Nodes with no help line.
    pub none: usize,
}

/// Count the help of an export made by [`case_schema_ui`].
pub fn help_coverage(schema: &Value) -> HelpCoverage {
    let mut c = HelpCoverage::default();
    for p in schema_paths(schema) {
        let Some(n) = node_at(schema, &p) else { continue };
        let leaf = n.get("properties").is_none() && n.get("x-rcfd-named-objects").is_none();
        c.nodes += 1;
        c.leaves += usize::from(leaf);
        match (
            n.get("x-rcfd-help").is_some(),
            n.get("x-rcfd-help-from").and_then(Value::as_str),
        ) {
            (false, _) => c.none += 1,
            (true, Some("docs")) => c.docs += 1,
            (true, Some(_)) => c.schema += 1,
            (true, None) => {
                c.written += 1;
                c.written_leaves += usize::from(leaf);
            }
        }
    }
    c
}

// ---- help.*

/// One row of `docs/glossary.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    /// The term.
    pub term: String,
    /// What it means.
    pub meaning: String,
    /// Where it is used most (Markdown, as the page writes it).
    pub page: String,
}

/// The header row the glossary table must keep (a changed shape fails ADR 0730 T7).
pub const GLOSSARY_HEADER: &str = "| Term | Meaning | Where it is used most |";

/// The rows of the glossary table of a page shaped like `docs/glossary.md`.
pub fn parse_glossary(md: &str) -> Result<Vec<Term>, String> {
    let mut lines = md.lines().skip_while(|l| l.trim() != GLOSSARY_HEADER);
    lines
        .next()
        .ok_or_else(|| format!("glossary.md: no table with the header {GLOSSARY_HEADER}"))?;
    let sep = lines.next().unwrap_or("");
    if !sep.trim_start().starts_with("| ---") {
        return Err("glossary.md: the table header is not followed by its separator row".into());
    }
    let mut out = Vec::new();
    for l in lines.take_while(|l| l.trim_start().starts_with('|')) {
        let cells: Vec<&str> = l.trim().trim_matches('|').split(" | ").map(str::trim).collect();
        let [term, meaning, page] = cells[..] else {
            return Err(format!("glossary.md: a row without three cells: {l}"));
        };
        out.push(Term {
            term: term.into(),
            meaning: meaning.into(),
            page: page.into(),
        });
    }
    Ok(out)
}

/// The reply of `help.glossary` and the text of `rcfd help glossary`.
pub fn glossary_json() -> Result<Value, String> {
    let terms = parse_glossary(GLOSSARY_MD)?;
    Ok(json!({
        "source": "docs/glossary.md",
        "terms": terms
            .iter()
            .map(|t| json!({"term": t.term, "meaning": t.meaning, "page": t.page}))
            .collect::<Vec<_>>(),
    }))
}

fn no_params(method: &str, p: &Value) -> Rpc<()> {
    match p {
        Value::Null => Ok(()),
        Value::Object(m) if m.is_empty() => Ok(()),
        _ => Err(RpcError::params(format!("{method} takes no parameters ({{}})"))),
    }
}

/// The `help.*` methods (only reached when the UI switch is on).
pub(crate) fn call(method: &str, p: &Value) -> Rpc {
    match method {
        "help.glossary" => {
            no_params(method, p)?;
            glossary_json().map_err(|e| RpcError::new(code::INTERNAL, e))
        }
        "help.errors" => {
            no_params(method, p)?;
            Ok(crate::errors::catalogue_json())
        }
        other => Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("unknown method '{other}'"),
        )),
    }
}

/// `settings.schema` of a service with the UI switch on: `{}` or `{ui: false}` is the reply of protocol 1.1, `{ui: true}`
/// adds the metadata; `ui` that is not a boolean, or parameters that are not an object, are -32602.
pub(crate) fn settings_schema(p: &Value, plain: impl FnOnce() -> Value) -> Rpc {
    let ui = match p {
        Value::Null => false,
        Value::Object(m) => match m.get("ui") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(RpcError::params("parameter 'ui' must be a boolean")),
        },
        _ => return Err(RpcError::params("settings.schema takes an object ({} or {ui: true})")),
    };
    if !ui {
        return Ok(plain());
    }
    let case = case_schema_ui().map_err(|e| RpcError::new(code::INTERNAL, e))?;
    Ok(json!({"case": case, "monitors": rcfd_post::monitors::monitor_json_schema()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_follow_github() {
        assert_eq!(slug("`turbulence`"), "turbulence");
        assert_eq!(
            slug("Mesh generators and boundary-layer inflation"),
            "mesh-generators-and-boundary-layer-inflation"
        );
        assert_eq!(
            slug("Landing on `solver.end_time` (ADR 0145)"),
            "landing-on-solverend_time-adr-0145"
        );
    }

    #[test]
    fn first_sentences() {
        assert_eq!(first_sentence("Cells in x").as_deref(), Some("Cells in x"));
        assert_eq!(
            first_sentence("Use `a`, e.g. b. Then more.").as_deref(),
            Some("Use a, e.g. b.")
        );
        assert_eq!(
            first_sentence("See [the page](x.md) now. Next").as_deref(),
            Some("See the page now.")
        );
        assert_eq!(first_sentence("| a | b |"), None);
        assert_eq!(first_sentence(&"word ".repeat(40)), None);
        assert_eq!(first_sentence("\n\nTwo\nlines. Rest").as_deref(), Some("Two lines."));
    }
}
