//! The settings state tree.
//!
//! A [`Schema`] describes typed objects, named-object collections (e.g. a map
//! of boundary names to boundary settings), lists, enums with allowed values,
//! dimensioned floats and commands. A [`SettingsTree`] holds the data (as a
//! `serde_json::Value`, always validated against the schema), supports
//! get/set by dotted path, command dispatch, serialization round trips,
//! JSON Schema export and a stable content hash.
//!
//! Dimensioned floats are stored in SI. A string such as `"10 km/h"` is
//! accepted wherever a velocity is expected and converted on `set`; a value
//! with the wrong dimension is rejected.

use crate::error::{Error, Result};
use crate::hash::fnv64;
use crate::units::{Dimension, Quantity};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Kind of a settings node.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Boolean.
    Bool {
        /// Default value.
        default: bool,
    },
    /// Integer with optional bounds.
    Int {
        /// Default value.
        default: i64,
        /// Inclusive lower bound.
        min: Option<i64>,
        /// Inclusive upper bound.
        max: Option<i64>,
    },
    /// Float with optional bounds and optional physical dimension (stored in SI).
    Float {
        /// Default value (SI).
        default: f64,
        /// Inclusive lower bound (SI).
        min: Option<f64>,
        /// Inclusive upper bound (SI).
        max: Option<f64>,
        /// Physical dimension, if any.
        dim: Option<Dimension>,
    },
    /// Free string.
    Str {
        /// Default value.
        default: String,
    },
    /// String restricted to allowed values.
    Enum {
        /// Default value (must be in `allowed`).
        default: String,
        /// Allowed values.
        allowed: Vec<String>,
    },
    /// Homogeneous list.
    List(Box<Schema>),
    /// Typed object with fixed fields.
    Object(Vec<Field>),
    /// Collection of user-named objects that all share one schema.
    Named(Box<Schema>),
    /// Command with typed arguments (not stored in the data).
    Command(Vec<Field>),
}

/// A named field of an object schema.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Field name.
    pub name: String,
    /// Field schema.
    pub schema: Schema,
}

/// Schema node: a kind plus documentation.
#[derive(Debug, Clone, PartialEq)]
pub struct Schema {
    /// Node kind.
    pub kind: Kind,
    /// Human readable documentation.
    pub doc: String,
    /// As a field of an object: left out of [`SettingsTree::content_hash`] while its value equals its default, so that adding
    /// the field to a schema leaves the hash of all earlier data unchanged (ADR 0156 D6). Default `false`.
    pub hash_omit_default: bool,
}

impl Schema {
    fn of(kind: Kind) -> Self {
        Schema {
            kind,
            doc: String::new(),
            hash_omit_default: false,
        }
    }
    /// Boolean setting.
    pub fn boolean(default: bool) -> Self {
        Self::of(Kind::Bool { default })
    }
    /// Integer setting.
    pub fn int(default: i64) -> Self {
        Self::of(Kind::Int {
            default,
            min: None,
            max: None,
        })
    }
    /// Float setting.
    pub fn float(default: f64) -> Self {
        Self::of(Kind::Float {
            default,
            min: None,
            max: None,
            dim: None,
        })
    }
    /// String setting.
    pub fn string(default: &str) -> Self {
        Self::of(Kind::Str {
            default: default.into(),
        })
    }
    /// Enum setting; `default` must be one of `allowed`.
    pub fn enumeration(default: &str, allowed: &[&str]) -> Self {
        assert!(
            allowed.contains(&default),
            "enum default '{default}' not in allowed values"
        );
        Self::of(Kind::Enum {
            default: default.into(),
            allowed: allowed.iter().map(|s| s.to_string()).collect(),
        })
    }
    /// List of items.
    pub fn list(item: Schema) -> Self {
        Self::of(Kind::List(Box::new(item)))
    }
    /// Empty object; add fields with [`Schema::field`].
    pub fn object() -> Self {
        Self::of(Kind::Object(Vec::new()))
    }
    /// Collection of named objects, each described by `item`.
    pub fn named(item: Schema) -> Self {
        Self::of(Kind::Named(Box::new(item)))
    }
    /// Command with the given arguments.
    pub fn command(args: Vec<Field>) -> Self {
        Self::of(Kind::Command(args))
    }
    /// Add a field to an object (or command) schema.
    pub fn field(mut self, name: &str, schema: Schema) -> Self {
        match &mut self.kind {
            Kind::Object(f) | Kind::Command(f) => f.push(Field {
                name: name.into(),
                schema,
            }),
            _ => panic!("field() on non-object schema"),
        }
        self
    }
    /// Attach documentation.
    pub fn doc(mut self, d: &str) -> Self {
        self.doc = d.into();
        self
    }
    /// Leave this field out of [`SettingsTree::content_hash`] while it holds its default value (ADR 0156 D6): for a key added
    /// to an existing schema, so that every earlier case keeps its hash.
    pub fn hash_omit_default(mut self) -> Self {
        self.hash_omit_default = true;
        self
    }
    /// Set bounds (in SI for floats) on an int or float.
    pub fn range(mut self, lo: f64, hi: f64) -> Self {
        match &mut self.kind {
            Kind::Float { min, max, .. } => {
                *min = Some(lo);
                *max = Some(hi);
            }
            Kind::Int { min, max, .. } => {
                *min = Some(lo as i64);
                *max = Some(hi as i64);
            }
            _ => panic!("range() on non-numeric schema"),
        }
        self
    }
    /// Set only a lower bound.
    pub fn at_least(mut self, lo: f64) -> Self {
        match &mut self.kind {
            Kind::Float { min, .. } => *min = Some(lo),
            Kind::Int { min, .. } => *min = Some(lo as i64),
            _ => panic!("at_least() on non-numeric schema"),
        }
        self
    }
    /// Declare the physical dimension of a float setting (value stored in SI).
    pub fn dim(mut self, d: Dimension) -> Self {
        match &mut self.kind {
            Kind::Float { dim, .. } => *dim = Some(d),
            _ => panic!("dim() on non-float schema"),
        }
        self
    }

    /// Default value for this schema.
    pub fn default_value(&self) -> Value {
        match &self.kind {
            Kind::Bool { default } => json!(default),
            Kind::Int { default, .. } => json!(default),
            Kind::Float { default, .. } => json!(default),
            Kind::Str { default } | Kind::Enum { default, .. } => json!(default),
            Kind::List(_) => json!([]),
            Kind::Named(_) => json!({}),
            Kind::Object(fields) => {
                let mut m = Map::new();
                for f in fields {
                    if !matches!(f.schema.kind, Kind::Command(_)) {
                        m.insert(f.name.clone(), f.schema.default_value());
                    }
                }
                Value::Object(m)
            }
            Kind::Command(_) => Value::Null,
        }
    }

    fn type_name(&self) -> &'static str {
        match &self.kind {
            Kind::Bool { .. } => "boolean",
            Kind::Int { .. } => "integer",
            Kind::Float { .. } => "number",
            Kind::Str { .. } | Kind::Enum { .. } => "string",
            Kind::List(_) => "list",
            Kind::Object(_) | Kind::Named(_) => "object",
            Kind::Command(_) => "command",
        }
    }

    /// Export as a JSON Schema (draft 2020-12 subset) document.
    pub fn to_json_schema(&self) -> Value {
        let mut v = self.json_schema_node();
        if let Value::Object(m) = &mut v {
            m.insert("$schema".into(), json!("https://json-schema.org/draft/2020-12/schema"));
        }
        v
    }

    fn json_schema_node(&self) -> Value {
        let mut m = Map::new();
        if !self.doc.is_empty() {
            m.insert("description".into(), json!(self.doc));
        }
        match &self.kind {
            Kind::Bool { default } => {
                m.insert("type".into(), json!("boolean"));
                m.insert("default".into(), json!(default));
            }
            Kind::Int { default, min, max } => {
                m.insert("type".into(), json!("integer"));
                m.insert("default".into(), json!(default));
                if let Some(x) = min {
                    m.insert("minimum".into(), json!(x));
                }
                if let Some(x) = max {
                    m.insert("maximum".into(), json!(x));
                }
            }
            Kind::Float { default, min, max, dim } => {
                m.insert("type".into(), json!("number"));
                m.insert("default".into(), json!(default));
                if let Some(x) = min {
                    m.insert("minimum".into(), json!(x));
                }
                if let Some(x) = max {
                    m.insert("maximum".into(), json!(x));
                }
                if let Some(d) = dim {
                    m.insert("x-rcfd-si-dimension".into(), json!(d.to_string()));
                    m.insert("x-rcfd-dimension-exponents".into(), json!(d.0));
                }
            }
            Kind::Str { default } => {
                m.insert("type".into(), json!("string"));
                m.insert("default".into(), json!(default));
            }
            Kind::Enum { default, allowed } => {
                m.insert("type".into(), json!("string"));
                m.insert("enum".into(), json!(allowed));
                m.insert("default".into(), json!(default));
            }
            Kind::List(item) => {
                m.insert("type".into(), json!("array"));
                m.insert("items".into(), item.json_schema_node());
            }
            Kind::Named(item) => {
                m.insert("type".into(), json!("object"));
                m.insert("additionalProperties".into(), item.json_schema_node());
                m.insert("x-rcfd-named-objects".into(), json!(true));
            }
            Kind::Object(fields) => {
                m.insert("type".into(), json!("object"));
                let mut props = Map::new();
                let mut cmds = Map::new();
                let mut required = Vec::new();
                for f in fields {
                    if let Kind::Command(args) = &f.schema.kind {
                        let mut a = Map::new();
                        for arg in args {
                            a.insert(arg.name.clone(), arg.schema.json_schema_node());
                        }
                        cmds.insert(
                            f.name.clone(),
                            json!({"description": f.schema.doc, "arguments": Value::Object(a)}),
                        );
                    } else {
                        props.insert(f.name.clone(), f.schema.json_schema_node());
                        required.push(f.name.clone());
                    }
                }
                m.insert("properties".into(), Value::Object(props));
                m.insert("required".into(), json!(required));
                m.insert("additionalProperties".into(), json!(false));
                if !cmds.is_empty() {
                    m.insert("x-rcfd-commands".into(), Value::Object(cmds));
                }
            }
            Kind::Command(_) => {}
        }
        Value::Object(m)
    }
}

/// Convenience constructor for a command argument field.
pub fn arg(name: &str, schema: Schema) -> Field {
    Field {
        name: name.into(),
        schema,
    }
}

fn invalid(path: &str, reason: impl Into<String>) -> Error {
    Error::InvalidValue {
        path: path.into(),
        reason: reason.into(),
    }
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Validate `value` against `schema` and normalise it (e.g. unit strings to SI).
pub fn coerce(schema: &Schema, value: &Value, path: &str) -> Result<Value> {
    match &schema.kind {
        Kind::Bool { .. } => match value {
            Value::Bool(_) => Ok(value.clone()),
            _ => Err(invalid(path, format!("expected boolean, got {}", value))),
        },
        Kind::Int { min, max, .. } => {
            let i = value
                .as_i64()
                .ok_or_else(|| invalid(path, format!("expected integer, got {}", value)))?;
            if min.is_some_and(|m| i < m) || max.is_some_and(|m| i > m) {
                return Err(invalid(path, format!("{i} outside [{:?}, {:?}]", min, max)));
            }
            Ok(json!(i))
        }
        Kind::Float { min, max, dim, .. } => {
            let x = match (value, dim) {
                (Value::Number(n), _) => n.as_f64().unwrap_or(f64::NAN),
                (Value::String(s), Some(d)) => {
                    let q = Quantity::parse(s).map_err(|e| invalid(path, e.to_string()))?;
                    q.expect(*d).map_err(|e| invalid(path, e.to_string()))?
                }
                (Value::String(s), None) => {
                    let q = Quantity::parse(s).map_err(|e| invalid(path, e.to_string()))?;
                    q.expect(Dimension::NONE).map_err(|e| invalid(path, e.to_string()))?
                }
                _ => return Err(invalid(path, format!("expected number, got {}", value))),
            };
            if !x.is_finite() {
                return Err(invalid(path, "value must be finite"));
            }
            if min.is_some_and(|m| x < m) || max.is_some_and(|m| x > m) {
                return Err(invalid(path, format!("{x} outside [{:?}, {:?}]", min, max)));
            }
            Ok(json!(x))
        }
        Kind::Str { .. } => match value {
            Value::String(_) => Ok(value.clone()),
            _ => Err(invalid(path, format!("expected string, got {}", value))),
        },
        Kind::Enum { allowed, .. } => match value.as_str() {
            Some(s) if allowed.iter().any(|a| a == s) => Ok(value.clone()),
            Some(s) => Err(invalid(path, format!("'{s}' is not one of {:?}", allowed))),
            None => Err(invalid(path, format!("expected one of {:?}", allowed))),
        },
        Kind::List(item) => {
            let arr = value.as_array().ok_or_else(|| invalid(path, "expected list"))?;
            let out: Result<Vec<Value>> = arr
                .iter()
                .enumerate()
                .map(|(i, v)| coerce(item, v, &format!("{path}.{i}")))
                .collect();
            Ok(Value::Array(out?))
        }
        Kind::Named(item) => {
            let obj = value
                .as_object()
                .ok_or_else(|| invalid(path, "expected object of named items"))?;
            let mut out = Map::new();
            for (k, v) in obj {
                if !valid_name(k) {
                    return Err(invalid(path, format!("invalid object name '{k}'")));
                }
                out.insert(k.clone(), coerce(item, v, &join(path, k))?);
            }
            Ok(Value::Object(out))
        }
        Kind::Object(fields) => {
            let obj = value.as_object().ok_or_else(|| invalid(path, "expected object"))?;
            let mut out = Map::new();
            for k in obj.keys() {
                if !fields
                    .iter()
                    .any(|f| &f.name == k && !matches!(f.schema.kind, Kind::Command(_)))
                {
                    return Err(invalid(path, format!("unknown setting '{k}'")));
                }
            }
            for f in fields {
                if matches!(f.schema.kind, Kind::Command(_)) {
                    continue;
                }
                let child = join(path, &f.name);
                match obj.get(&f.name) {
                    Some(v) => out.insert(f.name.clone(), coerce(&f.schema, v, &child)?),
                    None => out.insert(f.name.clone(), f.schema.default_value()),
                };
            }
            Ok(Value::Object(out))
        }
        Kind::Command(_) => Err(invalid(path, "commands hold no value; use call()")),
    }
}

fn join(path: &str, seg: &str) -> String {
    if path.is_empty() {
        seg.to_string()
    } else {
        format!("{path}.{seg}")
    }
}

type Handler = Arc<dyn Fn(&mut SettingsTree, &str, &Value) -> Result<Value> + Send + Sync>;

/// Settings tree: schema + validated data + command handlers.
#[derive(Clone)]
pub struct SettingsTree {
    schema: Schema,
    data: Value,
    handlers: BTreeMap<String, Handler>,
}

impl std::fmt::Debug for SettingsTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsTree").field("data", &self.data).finish()
    }
}

fn segments(path: &str) -> Vec<&str> {
    if path.is_empty() {
        vec![]
    } else {
        path.split('.').collect()
    }
}

impl SettingsTree {
    /// New tree filled with schema defaults.
    pub fn new(schema: Schema) -> Self {
        assert!(matches!(schema.kind, Kind::Object(_)), "root schema must be an object");
        let data = schema.default_value();
        SettingsTree {
            schema,
            data,
            handlers: BTreeMap::new(),
        }
    }

    /// Build a tree from previously serialized data, validating it.
    pub fn from_json(schema: Schema, data: &Value) -> Result<Self> {
        let mut t = SettingsTree::new(schema);
        t.data = coerce(&t.schema, data, "")?;
        Ok(t)
    }

    /// The schema.
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    /// Serialize the data to JSON.
    pub fn to_json(&self) -> Value {
        self.data.clone()
    }

    /// Serialize to a pretty JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(&self.data).expect("settings data is always serializable")
    }

    /// Stable content hash of the data (FNV-1a over canonical, key-sorted JSON). Object fields whose schema has
    /// [`Schema::hash_omit_default`] are left out while they hold their default (ADR 0156 D6); with no such field in the schema
    /// the hashed text is the serialised data itself.
    pub fn content_hash(&self) -> u64 {
        if !has_hash_omitted(&self.schema) {
            return fnv64(serde_json::to_string(&self.data).expect("serializable").as_bytes());
        }
        let pruned = hash_view(&self.schema, &self.data);
        fnv64(serde_json::to_string(&pruned).expect("serializable").as_bytes())
    }

    /// Schema node at `path` (named-object segments match any name).
    pub fn schema_at(&self, path: &str) -> Result<&Schema> {
        let mut s = &self.schema;
        for seg in segments(path) {
            s = child_schema(s, seg).ok_or_else(|| Error::PathNotFound(path.into()))?;
        }
        Ok(s)
    }

    /// Get the value at a dotted path (e.g. `"boundaries.inlet.velocity"`).
    pub fn get(&self, path: &str) -> Result<Value> {
        let mut v = &self.data;
        let mut s = &self.schema;
        for seg in segments(path) {
            s = child_schema(s, seg).ok_or_else(|| Error::PathNotFound(path.into()))?;
            if matches!(s.kind, Kind::Command(_)) {
                return Err(Error::PathNotFound(format!("{path} (is a command)")));
            }
            v = child_value(v, seg).ok_or_else(|| Error::PathNotFound(path.into()))?;
        }
        Ok(v.clone())
    }

    /// Set the value at a dotted path after validation and unit conversion.
    pub fn set(&mut self, path: &str, value: Value) -> Result<()> {
        let segs = segments(path);
        if segs.is_empty() {
            self.data = coerce(&self.schema, &value, "")?;
            return Ok(());
        }
        let schema = self.schema_at(path)?.clone();
        let new = coerce(&schema, &value, path)?;
        let mut cur = &mut self.data;
        for (i, seg) in segs.iter().enumerate() {
            let last = i + 1 == segs.len();
            let slot = child_value_mut(cur, seg).ok_or_else(|| Error::PathNotFound(path.into()))?;
            if last {
                *slot = new;
                return Ok(());
            }
            cur = slot;
        }
        Ok(())
    }

    /// Get a dimensioned float as a [`Quantity`].
    pub fn get_quantity(&self, path: &str) -> Result<Quantity> {
        let dim = match &self.schema_at(path)?.kind {
            Kind::Float { dim, .. } => dim.unwrap_or(Dimension::NONE),
            _ => return Err(invalid(path, "not a float setting")),
        };
        let x = self.get(path)?.as_f64().ok_or_else(|| invalid(path, "not a number"))?;
        Ok(Quantity::from_si(x, dim))
    }

    /// Create a named object (initialised with defaults) inside a named collection.
    pub fn add_named(&mut self, collection_path: &str, name: &str) -> Result<()> {
        if !valid_name(name) {
            return Err(invalid(collection_path, format!("invalid object name '{name}'")));
        }
        let item_default = match &self.schema_at(collection_path)?.kind {
            Kind::Named(item) => item.default_value(),
            _ => return Err(invalid(collection_path, "not a named-object collection")),
        };
        let mut coll = self.get(collection_path)?;
        let obj = coll.as_object_mut().expect("named collection is an object");
        if obj.contains_key(name) {
            return Err(invalid(collection_path, format!("'{name}' already exists")));
        }
        obj.insert(name.into(), item_default);
        self.set(collection_path, coll)
    }

    /// Remove a named object.
    pub fn remove_named(&mut self, collection_path: &str, name: &str) -> Result<()> {
        let mut coll = self.get(collection_path)?;
        let obj = coll
            .as_object_mut()
            .ok_or_else(|| invalid(collection_path, "not a collection"))?;
        if obj.remove(name).is_none() {
            return Err(Error::PathNotFound(join(collection_path, name)));
        }
        self.set(collection_path, coll)
    }

    /// Names present in a named collection (sorted).
    pub fn names(&self, collection_path: &str) -> Result<Vec<String>> {
        match self.get(collection_path)? {
            Value::Object(m) => Ok(m.keys().cloned().collect()),
            _ => Err(invalid(collection_path, "not a collection")),
        }
    }

    /// Allowed values of an enum setting.
    pub fn allowed_values(&self, path: &str) -> Result<Vec<String>> {
        match &self.schema_at(path)?.kind {
            Kind::Enum { allowed, .. } => Ok(allowed.clone()),
            _ => Err(invalid(path, "not an enum setting")),
        }
    }

    /// Child names of an object node (settings and commands), for browsing.
    pub fn children(&self, path: &str) -> Result<Vec<String>> {
        match &self.schema_at(path)?.kind {
            Kind::Object(f) => Ok(f.iter().map(|f| f.name.clone()).collect()),
            Kind::Named(_) => self.names(path),
            _ => Ok(vec![]),
        }
    }

    /// Short type name of the node at `path` (for UIs).
    pub fn type_of(&self, path: &str) -> Result<&'static str> {
        Ok(self.schema_at(path)?.type_name())
    }

    /// Register a handler for the command at schema path `pattern`, where
    /// named-collection segments are written `*` (e.g. `"boundaries.*.reset"`).
    /// The handler receives the tree, the concrete parent path and the
    /// validated arguments.
    pub fn register_command<F>(&mut self, pattern: &str, f: F)
    where
        F: Fn(&mut SettingsTree, &str, &Value) -> Result<Value> + Send + Sync + 'static,
    {
        self.handlers.insert(pattern.to_string(), Arc::new(f));
    }

    /// Invoke the command at `path` with JSON arguments.
    pub fn call(&mut self, path: &str, args: &Value) -> Result<Value> {
        let segs = segments(path);
        let (cmd, parent) = segs.split_last().ok_or_else(|| Error::PathNotFound(path.into()))?;
        // Walk the schema to build the normalised pattern.
        let mut s = &self.schema;
        let mut pattern = Vec::new();
        for seg in &segs {
            let was_named = matches!(s.kind, Kind::Named(_));
            s = child_schema(s, seg).ok_or_else(|| Error::PathNotFound(path.into()))?;
            pattern.push(if was_named { "*" } else { *seg });
        }
        let arg_fields = match &s.kind {
            Kind::Command(a) => a.clone(),
            _ => return Err(invalid(path, "not a command")),
        };
        let arg_schema = Schema::of(Kind::Object(arg_fields));
        let empty = json!({});
        let args = coerce(&arg_schema, if args.is_null() { &empty } else { args }, path)?;
        let key = pattern.join(".");
        let h = self
            .handlers
            .get(&key)
            .cloned()
            .ok_or_else(|| invalid(path, format!("no handler registered for command '{key}'")))?;
        let _ = cmd;
        h(self, &parent.join("."), &args)
    }
}

/// True when a field anywhere below `s` has [`Schema::hash_omit_default`] (ADR 0156 D6).
fn has_hash_omitted(s: &Schema) -> bool {
    match &s.kind {
        Kind::Object(fields) => fields
            .iter()
            .any(|f| f.schema.hash_omit_default || has_hash_omitted(&f.schema)),
        Kind::Named(item) | Kind::List(item) => has_hash_omitted(item),
        _ => false,
    }
}

/// The data as [`SettingsTree::content_hash`] hashes it: every object field with [`Schema::hash_omit_default`] that holds its
/// default is left out; everything else is kept, in the data's own order (so the text equals the text of the data without
/// those fields).
fn hash_view(s: &Schema, v: &Value) -> Value {
    match (&s.kind, v) {
        (Kind::Object(fields), Value::Object(m)) => {
            let mut out = Map::new();
            for (k, x) in m {
                match fields.iter().find(|f| &f.name == k) {
                    Some(f) if f.schema.hash_omit_default && *x == f.schema.default_value() => {}
                    Some(f) => {
                        out.insert(k.clone(), hash_view(&f.schema, x));
                    }
                    None => {
                        out.insert(k.clone(), x.clone());
                    }
                }
            }
            Value::Object(out)
        }
        (Kind::Named(item), Value::Object(m)) => {
            Value::Object(m.iter().map(|(k, x)| (k.clone(), hash_view(item, x))).collect())
        }
        (Kind::List(item), Value::Array(a)) => Value::Array(a.iter().map(|x| hash_view(item, x)).collect()),
        _ => v.clone(),
    }
}

fn child_schema<'a>(s: &'a Schema, seg: &str) -> Option<&'a Schema> {
    match &s.kind {
        Kind::Object(f) => f.iter().find(|f| f.name == seg).map(|f| &f.schema),
        Kind::Named(item) => Some(item),
        Kind::List(item) => seg.parse::<usize>().ok().map(|_| &**item),
        _ => None,
    }
}

fn child_value<'a>(v: &'a Value, seg: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => m.get(seg),
        Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get(i)),
        _ => None,
    }
}

fn child_value_mut<'a>(v: &'a mut Value, seg: &str) -> Option<&'a mut Value> {
    match v {
        Value::Object(m) => m.get_mut(seg),
        Value::Array(a) => seg.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
        _ => None,
    }
}

/// Validate `data` against a JSON Schema document produced by
/// [`Schema::to_json_schema`] (supports type, properties, required,
/// additionalProperties, enum, minimum, maximum, items). Returns all errors.
pub fn validate_json_schema(schema: &Value, data: &Value) -> std::result::Result<(), Vec<String>> {
    let mut errs = Vec::new();
    check_json(schema, data, "", &mut errs);
    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}

fn check_json(schema: &Value, data: &Value, path: &str, errs: &mut Vec<String>) {
    let Some(s) = schema.as_object() else { return };
    if let Some(t) = s.get("type").and_then(Value::as_str) {
        let ok = match t {
            "object" => data.is_object(),
            "array" => data.is_array(),
            "string" => data.is_string(),
            "boolean" => data.is_boolean(),
            "integer" => data.is_i64() || data.is_u64(),
            "number" => data.is_number(),
            _ => true,
        };
        if !ok {
            errs.push(format!("{path}: expected {t}"));
            return;
        }
    }
    if let Some(e) = s.get("enum").and_then(Value::as_array) {
        if !e.contains(data) {
            errs.push(format!("{path}: {data} not in enum"));
        }
    }
    if let (Some(x), Some(m)) = (data.as_f64(), s.get("minimum").and_then(Value::as_f64)) {
        if x < m {
            errs.push(format!("{path}: {x} < minimum {m}"));
        }
    }
    if let (Some(x), Some(m)) = (data.as_f64(), s.get("maximum").and_then(Value::as_f64)) {
        if x > m {
            errs.push(format!("{path}: {x} > maximum {m}"));
        }
    }
    if let Some(obj) = data.as_object() {
        let props = s.get("properties").and_then(Value::as_object);
        if let Some(req) = s.get("required").and_then(Value::as_array) {
            for r in req.iter().filter_map(Value::as_str) {
                if !obj.contains_key(r) {
                    errs.push(format!("{path}: missing required '{r}'"));
                }
            }
        }
        for (k, v) in obj {
            let p = join(path, k);
            if let Some(ps) = props.and_then(|p| p.get(k)) {
                check_json(ps, v, &p, errs);
            } else {
                match s.get("additionalProperties") {
                    Some(Value::Bool(false)) => errs.push(format!("{p}: unexpected property")),
                    Some(sub @ Value::Object(_)) => check_json(sub, v, &p, errs),
                    _ => {}
                }
            }
        }
    }
    if let (Some(arr), Some(items)) = (data.as_array(), s.get("items")) {
        for (i, v) in arr.iter().enumerate() {
            check_json(items, v, &format!("{path}.{i}"), errs);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_schema() -> Schema {
        Schema::object()
            .field(
                "solver",
                Schema::object()
                    .field("scheme", Schema::enumeration("SIMPLE", &["SIMPLE", "SIMPLEC", "PISO"]))
                    .field("max_iterations", Schema::int(100).range(1.0, 1e6))
                    .field("relax_p", Schema::float(0.3).range(0.0, 1.0)),
            )
            .field(
                "materials",
                Schema::named(
                    Schema::object()
                        .field("density", Schema::float(1.225).dim(Dimension::DENSITY).at_least(0.0))
                        .field("viscosity", Schema::float(1.8e-5).dim(Dimension::DYNAMIC_VISCOSITY)),
                ),
            )
            .field(
                "boundaries",
                Schema::named(
                    Schema::object()
                        .field(
                            "type",
                            Schema::enumeration("wall", &["wall", "velocity_inlet", "pressure_outlet"]),
                        )
                        .field("velocity", Schema::float(0.0).dim(Dimension::VELOCITY))
                        .field("reset", Schema::command(vec![arg("value", Schema::float(0.0))])),
                ),
            )
            .field("probes", Schema::list(Schema::float(0.0)))
            .field("initialize", Schema::command(vec![arg("fill", Schema::float(1.0))]))
    }

    #[test]
    fn defaults_and_get_set_by_path() {
        let mut t = SettingsTree::new(sample_schema());
        assert_eq!(t.get("solver.scheme").unwrap(), json!("SIMPLE"));
        t.set("solver.scheme", json!("PISO")).unwrap();
        assert_eq!(t.get("solver.scheme").unwrap(), json!("PISO"));
        t.set("solver.max_iterations", json!(500)).unwrap();
        assert_eq!(t.get("solver").unwrap()["max_iterations"], json!(500));
        assert!(matches!(t.get("solver.nope"), Err(Error::PathNotFound(_))));
        assert!(t.set("solver.relax_p", json!(1.5)).is_err());
        assert!(t.set("solver.scheme", json!("NOPE")).is_err());
        assert!(t.set("solver.max_iterations", json!(1.5)).is_err());
        assert_eq!(t.allowed_values("solver.scheme").unwrap().len(), 3);
        assert_eq!(
            t.children("solver").unwrap(),
            vec!["scheme", "max_iterations", "relax_p"]
        );
    }

    #[test]
    fn named_objects_and_dimension_checking() {
        let mut t = SettingsTree::new(sample_schema());
        t.add_named("boundaries", "inlet").unwrap();
        t.add_named("boundaries", "outlet").unwrap();
        assert!(t.add_named("boundaries", "inlet").is_err());
        assert!(t.add_named("boundaries", "bad name").is_err());
        assert_eq!(t.names("boundaries").unwrap(), vec!["inlet", "outlet"]);
        t.set("boundaries.inlet.type", json!("velocity_inlet")).unwrap();
        // unit string converted to SI
        t.set("boundaries.inlet.velocity", json!("36 km/h")).unwrap();
        let v = t.get_quantity("boundaries.inlet.velocity").unwrap();
        assert!((v.si - 10.0).abs() < 1e-12);
        assert_eq!(v.dim, Dimension::VELOCITY);
        // wrong dimension rejected
        let e = t.set("boundaries.inlet.velocity", json!("5 Pa")).unwrap_err();
        assert!(e.to_string().contains("dimension mismatch"), "{e}");
        // material properties
        t.add_named("materials", "water").unwrap();
        t.set("materials.water.density", json!("998 kg/m^3")).unwrap();
        t.set("materials.water.viscosity", json!("1 cP")).unwrap();
        assert!((t.get("materials.water.viscosity").unwrap().as_f64().unwrap() - 1e-3).abs() < 1e-15);
        assert!(t.set("materials.water.density", json!("-1 kg/m^3")).is_err());
        t.remove_named("boundaries", "outlet").unwrap();
        assert_eq!(t.names("boundaries").unwrap(), vec!["inlet"]);
        assert!(t.remove_named("boundaries", "outlet").is_err());
    }

    #[test]
    fn lists_by_index() {
        let mut t = SettingsTree::new(sample_schema());
        t.set("probes", json!([1.0, 2, "0.5"])).unwrap();
        assert_eq!(t.get("probes").unwrap(), json!([1.0, 2.0, 0.5]));
        assert_eq!(t.get("probes.2").unwrap(), json!(0.5));
        assert!(t.set("probes", json!([1.0, "1 m"])).is_err());
        assert!(t.get("probes.9").is_err());
    }

    #[test]
    fn round_trip_serialize() {
        let mut t = SettingsTree::new(sample_schema());
        t.add_named("boundaries", "wall1").unwrap();
        t.set("solver.relax_p", json!(0.7)).unwrap();
        t.set("probes", json!([1.0, 2.5])).unwrap();
        t.set("probes.1", json!(3.5)).unwrap();
        let text = t.to_json_string();
        let parsed: Value = serde_json::from_str(&text).unwrap();
        let t2 = SettingsTree::from_json(sample_schema(), &parsed).unwrap();
        assert_eq!(t.to_json(), t2.to_json());
        assert_eq!(t.content_hash(), t2.content_hash());
        t.set("solver.relax_p", json!(0.71)).unwrap();
        assert_ne!(t.content_hash(), t2.content_hash());
        // invalid data rejected
        let mut bad = parsed.clone();
        bad["solver"]["scheme"] = json!("XX");
        assert!(SettingsTree::from_json(sample_schema(), &bad).is_err());
        bad = parsed;
        bad["extra"] = json!(1);
        assert!(SettingsTree::from_json(sample_schema(), &bad).is_err());
    }

    /// ADR 0156 D6: a field added with `hash_omit_default` leaves the hash of data without it unchanged while it holds its
    /// default (also inside named objects), changes it at any other value, and is still a normal key everywhere else.
    #[test]
    fn hash_omitted_default_leaves_earlier_hashes_unchanged() {
        let with_key = |s: Schema| {
            let Kind::Object(mut fields) = s.kind else {
                unreachable!()
            };
            for f in fields.iter_mut() {
                if f.name == "solver" {
                    f.schema = f.schema.clone().field(
                        "mode",
                        Schema::enumeration("fixed", &["fixed", "adaptive"]).hash_omit_default(),
                    );
                }
                if f.name == "boundaries" {
                    let Kind::Named(item) = &f.schema.kind else {
                        unreachable!()
                    };
                    f.schema = Schema::named(
                        item.as_ref()
                            .clone()
                            .field("extra", Schema::float(2.5).hash_omit_default()),
                    );
                }
            }
            Schema::of(Kind::Object(fields))
        };
        let mut old = SettingsTree::new(sample_schema());
        old.add_named("boundaries", "inlet").unwrap();
        old.set("solver.relax_p", json!(0.7)).unwrap();
        let mut new = SettingsTree::new(with_key(sample_schema()));
        new.add_named("boundaries", "inlet").unwrap();
        new.set("solver.relax_p", json!(0.7)).unwrap();
        assert!(new.to_json()["solver"].get("mode").is_some(), "the key is in the data");
        assert_eq!(new.content_hash(), old.content_hash());
        // written out at its default: the same data, the same hash
        let mut explicit = new.clone();
        explicit.set("solver.mode", json!("fixed")).unwrap();
        explicit.set("boundaries.inlet.extra", json!(2.5)).unwrap();
        assert_eq!(explicit.content_hash(), old.content_hash());
        // another value: another hash
        let mut adaptive = new.clone();
        adaptive.set("solver.mode", json!("adaptive")).unwrap();
        assert_ne!(adaptive.content_hash(), old.content_hash());
        let mut extra = new.clone();
        extra.set("boundaries.inlet.extra", json!(3.0)).unwrap();
        assert_ne!(extra.content_hash(), old.content_hash());
        assert_ne!(extra.content_hash(), adaptive.content_hash());
        // the old schema's own hashing is the serialised data, unchanged
        assert_eq!(
            old.content_hash(),
            crate::hash::fnv64(serde_json::to_string(&old.to_json()).unwrap().as_bytes())
        );
    }

    #[test]
    fn json_schema_export_validates_sample() {
        let mut t = SettingsTree::new(sample_schema());
        t.add_named("boundaries", "inlet").unwrap();
        let js = t.schema().to_json_schema();
        assert_eq!(
            js["properties"]["solver"]["properties"]["scheme"]["enum"],
            json!(["SIMPLE", "SIMPLEC", "PISO"])
        );
        assert_eq!(
            js["properties"]["materials"]["additionalProperties"]["properties"]["density"]["x-rcfd-si-dimension"],
            json!("m^-3 kg")
        );
        assert!(js["x-rcfd-commands"]["initialize"].is_object());
        validate_json_schema(&js, &t.to_json()).unwrap();
        // negative: bad enum + out-of-range + unknown property
        let mut bad = t.to_json();
        bad["solver"]["scheme"] = json!("X");
        bad["solver"]["relax_p"] = json!(2.0);
        bad["junk"] = json!(1);
        let errs = validate_json_schema(&js, &bad).unwrap_err();
        assert_eq!(errs.len(), 3, "{errs:?}");
    }

    #[test]
    fn commands() {
        let mut t = SettingsTree::new(sample_schema());
        t.add_named("boundaries", "inlet").unwrap();
        t.register_command("boundaries.*.reset", |tree, parent, args| {
            tree.set(&format!("{parent}.velocity"), args["value"].clone())?;
            Ok(json!("ok"))
        });
        t.register_command("initialize", |_, _, args| Ok(args["fill"].clone()));
        t.set("boundaries.inlet.velocity", json!(4.0)).unwrap();
        assert_eq!(
            t.call("boundaries.inlet.reset", &json!({"value": 0.0})).unwrap(),
            json!("ok")
        );
        assert_eq!(t.get("boundaries.inlet.velocity").unwrap(), json!(0.0));
        assert_eq!(t.call("initialize", &Value::Null).unwrap(), json!(1.0));
        assert!(t.call("initialize", &json!({"fill": "x"})).is_err());
        assert!(t.call("solver.scheme", &Value::Null).is_err());
        assert!(t.get("initialize").is_err());
    }
}
