//! The case schema against its reference page (docs/process/documentation-process-review.md, improvement I5; no ADR number
//! exists for this work yet, the lead assigns it). `docs/case-schema.md` is kept by hand and compiled into the engine, so it
//! is not generated; this test compares it with `rcfd_api::schema::case_schema()` in both directions:
//!
//! - P1: every table of the schema (an object, or a collection of named objects) is named on the page in backticks by its
//!   dotted path, as `mesh.fan` or `[mesh.fan]`.
//! - P2: every dotted key in backticks on the page whose first part is a section of the schema resolves in the schema.
//!
//! The cases found when the test was written (2026-11-18) are the two lists below. The test fails on a new case of either
//! kind, and on a listed case that no longer applies, so the lists only shrink. No solver runs.

use rcfd_core::settings::{Kind, Schema};
use std::collections::BTreeSet;

/// P1 exceptions: the 12 schema tables (of 122) not named on the page when the test was written.
const UNNAMED_TABLES: &[&str] = &[
    "compressible.initial.left",
    "compressible.initial.right",
    "decomposition.weights",
    "expressions",
    "particles.injection",
    "scalars.*.boundaries",
    "species.*.boundaries",
    "turbulence.coefficients.k_epsilon",
    "turbulence.coefficients.k_omega_sst",
    "turbulence.coefficients.mixing_length",
    "turbulence.coefficients.sa",
    "turbulence.coefficients.wale",
];

/// P2 exceptions: the 8 dotted keys (of 460 on the page) that did not resolve in the schema when the test was written.
/// Several are service methods or commands written with a dot, not case keys.
const UNRESOLVED_KEYS: &[&str] = &[
    "cosim.describe",
    "mesh.check",
    "mesh.fan.mesh",
    "mesh.fan.station_<name>",
    "mesh.fan.stations",
    "mesh.inflate",
    "mesh.partition",
    "particles.report",
];

/// A segment of a table path that stands for a user-chosen name (a collection of named objects).
const ANY: &str = "*";

/// The dotted paths of every table below `s`; a named collection contributes its own path and `path.*` for its members.
fn tables(s: &Schema, path: &str, out: &mut BTreeSet<String>) {
    let join = |name: &str| {
        if path.is_empty() {
            name.to_string()
        } else {
            format!("{path}.{name}")
        }
    };
    match &s.kind {
        Kind::Object(fields) => {
            if !path.is_empty() {
                out.insert(path.to_string());
            }
            for f in fields {
                tables(&f.schema, &join(&f.name), out);
            }
        }
        Kind::Named(inner) => {
            out.insert(path.to_string());
            tables(inner, &join(ANY), out);
        }
        Kind::List(inner) => tables(inner, path, out),
        _ => {}
    }
}

/// Whether the dotted key `segs` names a node of the schema. A named collection accepts any one segment; a list is its
/// element, with or without a placeholder for the index; a placeholder segment `<x>` stands for any one field; a leaf
/// accepts no further segment.
fn resolves(s: &Schema, segs: &[&str]) -> bool {
    let Some((first, rest)) = segs.split_first() else {
        return true;
    };
    let placeholder = first.starts_with('<');
    match &s.kind {
        Kind::Object(fields) | Kind::Command(fields) => fields
            .iter()
            .any(|f| (placeholder || f.name == *first) && resolves(&f.schema, rest)),
        Kind::Named(inner) => resolves(inner, rest),
        Kind::List(inner) => resolves(inner, segs) || (placeholder && resolves(inner, rest)),
        _ => false,
    }
}

/// The text of every `code span` of the page outside fenced blocks.
fn spans(md: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut fence = false;
    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        let mut parts = line.split('`');
        parts.next();
        while let (Some(code), Some(_)) = (parts.next(), parts.next()) {
            out.push(code.to_string());
        }
    }
    out
}

/// The dotted keys in a code span: maximal runs of name characters and dots with at least one dot, such as `mesh.fan.blades`
/// in `[mesh.fan.blades]` or in `mesh.fan.blades = 5`. A `<name>` segment stays as written. Not keys, and left out: a
/// pattern with `*` (`fluid.*`) and a file name (`mesh.md`, `study.json`).
fn dotted(span: &str) -> Vec<String> {
    const FILE_ENDINGS: &[&str] = &[
        "md", "json", "toml", "rcfd", "vtp", "vtu", "vtk", "csv", "rs", "py", "msh", "stl",
    ];
    let ok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '<' | '>' | '*');
    span.split(|c: char| !ok(c))
        .map(|t| t.trim_matches('.'))
        .filter(|t| t.contains('.') && t.chars().next().is_some_and(|c| c.is_ascii_lowercase()))
        .filter(|t| t.split('.').all(|p| !p.is_empty()))
        .filter(|t| !t.contains('*') && !FILE_ENDINGS.contains(&t.rsplit('.').next().unwrap()))
        .map(str::to_string)
        .collect()
}

/// Whether the table path `table` (with `*` segments) is named by the page key `key`: equal segment by segment, a `*`
/// matching any segment, the key allowed to go on below the table.
fn names(key: &str, table: &str) -> bool {
    let (k, t): (Vec<&str>, Vec<&str>) = (key.split('.').collect(), table.split('.').collect());
    k.len() >= t.len() && t.iter().zip(&k).all(|(a, b)| *a == ANY || a == b)
}

#[test]
fn case_schema_and_its_page_name_the_same_tables_and_keys() {
    let schema = rcfd_api::schema::case_schema();
    let page = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/case-schema.md")).unwrap();
    let Kind::Object(top) = &schema.kind else {
        panic!("the case schema is an object")
    };
    let sections: BTreeSet<&str> = top.iter().map(|f| f.name.as_str()).collect();
    let all = spans(&page);
    let keys: BTreeSet<String> = all.iter().flat_map(|s| dotted(s)).collect();
    // A top-level section is named by its bare name or `[name]` as well.
    let bare: BTreeSet<&str> = all
        .iter()
        .map(|s| s.trim().trim_start_matches('[').trim_end_matches(']'))
        .collect();

    let mut t = BTreeSet::new();
    tables(&schema, "", &mut t);
    let unnamed: BTreeSet<String> = t
        .iter()
        .filter(|p| !p.ends_with(ANY))
        .filter(|p| !(bare.contains(p.as_str()) || keys.iter().any(|k| names(k, p))))
        .cloned()
        .collect();
    let unresolved: BTreeSet<String> = keys
        .iter()
        .filter(|k| sections.contains(k.split('.').next().unwrap()))
        .filter(|k| !resolves(&schema, &k.split('.').collect::<Vec<_>>()))
        .cloned()
        .collect();
    println!(
        "case schema page: {} tables in the schema, {} not named on the page; {} dotted keys on the page, {} unresolved",
        t.len(),
        unnamed.len(),
        keys.len(),
        unresolved.len()
    );

    let diff = |found: &BTreeSet<String>, listed: &[&str]| -> (Vec<String>, Vec<String>) {
        let listed: BTreeSet<String> = listed.iter().map(|s| s.to_string()).collect();
        (
            found.difference(&listed).cloned().collect(),
            listed.difference(found).cloned().collect(),
        )
    };
    let (new_tables, stale_tables) = diff(&unnamed, UNNAMED_TABLES);
    let (new_keys, stale_keys) = diff(&unresolved, UNRESOLVED_KEYS);
    assert!(
        new_tables.is_empty(),
        "P1: tables of the case schema that docs/case-schema.md does not name in backticks; describe them on the page: {new_tables:?}"
    );
    assert!(
        new_keys.is_empty(),
        "P2: dotted keys in backticks on docs/case-schema.md that are not in the case schema; correct the page: {new_keys:?}"
    );
    assert!(
        stale_tables.is_empty(),
        "P1: remove from UNNAMED_TABLES, the page names them now: {stale_tables:?}"
    );
    assert!(
        stale_keys.is_empty(),
        "P2: remove from UNRESOLVED_KEYS, they resolve or are gone: {stale_keys:?}"
    );
}

#[test]
fn page_reading_rules() {
    assert_eq!(spans("a `x.y` b `[m.n]`\n```\n`in.fence`\n```\n"), vec!["x.y", "[m.n]"]);
    assert_eq!(dotted("[mesh.fan.blades]"), vec!["mesh.fan.blades"]);
    assert_eq!(dotted("solver.dt = 0.5 and case.toml, fluid.* too"), vec!["solver.dt"]);
    assert!(dotted("0.5 Mesh.x plain").is_empty());
    assert!(names("boundaries.inlet.type", "boundaries.*"));
    assert!(!names("boundaries", "boundaries.*"));
    let s = Schema::object().field("a", Schema::object().field("b", Schema::float(0.0)));
    assert!(resolves(&s, &["a", "b"]));
    assert!(!resolves(&s, &["a", "b", "c"]));
    assert!(!resolves(&s, &["a", "x"]));
    assert!(resolves(&s, &["a", "<key>"]));
    let l = Schema::object().field("rows", Schema::list(Schema::object().field("v", Schema::float(0.0))));
    assert!(
        resolves(&l, &["rows", "v"]) && resolves(&l, &["rows", "<i>", "v"]) && !resolves(&l, &["rows", "<i>", "w"])
    );
}
