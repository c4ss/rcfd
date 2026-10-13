//! Settings-tree model built from the case JSON Schema (`settings.schema`).
//!
//! No UI code: this turns the schema into a tree of typed nodes, parses and
//! range-checks user input locally, and resolves settings paths (including
//! named entries such as `boundaries.<patch>.velocity`) to their schema node.

use serde_json::Value;

/// What kind of value a node holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// true/false.
    Bool,
    /// Integer with optional bounds.
    Int {
        /// Lower bound.
        min: Option<f64>,
        /// Upper bound.
        max: Option<f64>,
    },
    /// Real number with optional bounds and SI dimension text.
    Float {
        /// Lower bound.
        min: Option<f64>,
        /// Upper bound.
        max: Option<f64>,
        /// SI dimension, when the schema states one.
        dim: Option<String>,
    },
    /// Free text.
    Text,
    /// One of a fixed set of strings.
    Enum(Vec<String>),
    /// A list of numbers (vectors).
    NumberList,
    /// An object with fixed fields.
    Group,
    /// A map of user-named objects of one shape (patches, reports).
    Named,
}

/// One node of the settings tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Field name (last path segment).
    pub name: String,
    /// Documentation string.
    pub doc: String,
    /// Kind.
    pub kind: Kind,
    /// Default value from the schema.
    pub default: Value,
    /// Fields of a `Group`, or the single item template of a `Named` map.
    pub children: Vec<Node>,
}

impl Node {
    fn leaf(name: &str, schema: &Value) -> Node {
        let min = schema.get("minimum").and_then(Value::as_f64);
        let max = schema.get("maximum").and_then(Value::as_f64);
        let kind = if let Some(e) = schema.get("enum").and_then(Value::as_array) {
            Kind::Enum(e.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        } else {
            match schema.get("type").and_then(Value::as_str) {
                Some("boolean") => Kind::Bool,
                Some("integer") => Kind::Int { min, max },
                Some("number") => Kind::Float {
                    min,
                    max,
                    dim: schema
                        .get("x-rcfd-si-dimension")
                        .and_then(Value::as_str)
                        .map(String::from),
                },
                Some("array") => Kind::NumberList,
                _ => Kind::Text,
            }
        };
        Node {
            name: name.to_string(),
            doc: schema
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            kind,
            default: schema.get("default").cloned().unwrap_or(Value::Null),
            children: vec![],
        }
    }

    /// Build a node (and its subtree) from a JSON Schema fragment.
    pub fn from_schema(name: &str, schema: &Value) -> Node {
        let named = schema.get("x-rcfd-named-objects") == Some(&Value::Bool(true));
        if named {
            let mut n = Node::leaf(name, schema);
            n.kind = Kind::Named;
            if let Some(item) = schema.get("additionalProperties").filter(|v| v.is_object()) {
                n.children = vec![Node::from_schema("<name>", item)];
            }
            return n;
        }
        if let Some(props) = schema.get("properties").and_then(Value::as_object) {
            let mut n = Node::leaf(name, schema);
            n.kind = Kind::Group;
            // The service sorts keys alphabetically; keep that order (stable and predictable).
            n.children = props.iter().map(|(k, v)| Node::from_schema(k, v)).collect();
            return n;
        }
        Node::leaf(name, schema)
    }

    /// True for leaves the user edits directly.
    pub fn is_leaf(&self) -> bool {
        !matches!(self.kind, Kind::Group | Kind::Named)
    }

    /// Number of editable leaves in this subtree (named templates count once).
    pub fn leaf_count(&self) -> usize {
        if self.is_leaf() {
            1
        } else {
            self.children.iter().map(Node::leaf_count).sum()
        }
    }

    /// Resolve a dotted settings path to its schema node. For named maps the
    /// segment after the map name is the entry name and maps to the item template.
    pub fn find(&self, path: &str) -> Option<&Node> {
        let mut cur = self;
        for s in path.split('.').filter(|s| !s.is_empty()) {
            match cur.kind {
                Kind::Group => cur = cur.children.iter().find(|c| c.name == s)?,
                Kind::Named => {
                    // `s` is an entry name; the item template describes its contents.
                    let _ = s;
                    cur = cur.children.first()?;
                }
                _ => return None,
            }
        }
        Some(cur)
    }

    /// All leaf paths of the fixed part of the tree (named maps are not expanded).
    pub fn leaf_paths(&self) -> Vec<String> {
        fn walk(n: &Node, prefix: &str, out: &mut Vec<String>) {
            for c in &n.children {
                let p = if prefix.is_empty() {
                    c.name.clone()
                } else {
                    format!("{prefix}.{}", c.name)
                };
                match c.kind {
                    Kind::Group => walk(c, &p, out),
                    Kind::Named => {}
                    _ => out.push(p),
                }
            }
        }
        let mut out = vec![];
        walk(self, "", &mut out);
        out
    }
}

/// Build the root node from the `case` part of `settings.schema`.
pub fn build_tree(case_schema: &Value) -> Node {
    Node::from_schema("case", case_schema)
}

/// Parse user text for a leaf of the given kind; returns the JSON value to send.
/// Range violations are rejected here with a message.
pub fn parse_input(kind: &Kind, text: &str) -> Result<Value, String> {
    let t = text.trim();
    match kind {
        Kind::Bool => match t.to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(Value::Bool(true)),
            "false" | "no" | "0" => Ok(Value::Bool(false)),
            _ => Err("expected true or false".into()),
        },
        Kind::Int { min, max } => {
            let v: i64 = t.parse().map_err(|_| format!("'{t}' is not a whole number"))?;
            check_range(v as f64, *min, *max)?;
            Ok(Value::from(v))
        }
        Kind::Float { min, max, .. } => {
            let v: f64 = t.parse().map_err(|_| format!("'{t}' is not a number"))?;
            if !v.is_finite() {
                return Err("value must be finite".into());
            }
            check_range(v, *min, *max)?;
            Ok(Value::from(v))
        }
        Kind::Text => Ok(Value::String(text.to_string())),
        Kind::Enum(allowed) => {
            if allowed.iter().any(|a| a == t) {
                Ok(Value::String(t.to_string()))
            } else {
                Err(format!("must be one of: {}", allowed.join(", ")))
            }
        }
        Kind::NumberList => {
            let inner = t.trim_start_matches('[').trim_end_matches(']');
            if inner.trim().is_empty() {
                return Ok(Value::Array(vec![]));
            }
            let mut out = vec![];
            for part in inner.split([',', ' ', ';']).filter(|p| !p.is_empty()) {
                let v: f64 = part.parse().map_err(|_| format!("'{part}' is not a number"))?;
                if !v.is_finite() {
                    return Err("values must be finite".into());
                }
                out.push(Value::from(v));
            }
            Ok(Value::Array(out))
        }
        Kind::Group | Kind::Named => Err("not an editable value".into()),
    }
}

fn check_range(v: f64, min: Option<f64>, max: Option<f64>) -> Result<(), String> {
    if let Some(m) = min {
        if v < m {
            return Err(format!("{v} is below the minimum {m}"));
        }
    }
    if let Some(m) = max {
        if v > m {
            return Err(format!("{v} is above the maximum {m}"));
        }
    }
    Ok(())
}

/// Text shown in an edit box for a value.
pub fn format_value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => format!("[{}]", a.iter().map(format_value).collect::<Vec<_>>().join(", ")),
        other => other.to_string(),
    }
}

/// Look up a dotted path in a settings value.
pub fn value_at<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for s in path.split('.').filter(|s| !s.is_empty()) {
        cur = cur.get(s)?;
    }
    Some(cur)
}

/// Does this subtree (or its name / doc) match the filter text? Empty filter matches all.
pub fn matches_filter(node: &Node, path: &str, filter: &str) -> bool {
    let f = filter.trim().to_lowercase();
    if f.is_empty() {
        return true;
    }
    path.to_lowercase().contains(&f)
        || node.doc.to_lowercase().contains(&f)
        || node
            .children
            .iter()
            .any(|c| matches_filter(c, &format!("{path}.{}", c.name), &f))
}
