//! ADR 0730 T3, T4, T5, T7 and the help coverage: the data file `src/schema_ui.toml` against the case schema and the
//! built-in docs, and `help.glossary` against `docs/glossary.md`.

use rcfd_api::schema::case_schema;
use rcfd_api::ui::{self, Metadata};
use serde_json::Value;

fn meta() -> &'static Metadata {
    ui::metadata().expect("schema_ui.toml parses")
}

fn export() -> Value {
    case_schema().to_json_schema()
}

/// The line of the data file that opens the entry of a path, for the messages.
fn line_of(path: &str) -> usize {
    let head = format!("[keys.\"{path}\"]");
    ui::METADATA_TOML
        .lines()
        .position(|l| l.trim() == head)
        .map_or(0, |i| i + 1)
}

/// T3: every path in the data file names a node of `case_schema()`; the schema paths it lacks are reported.
#[test]
fn t3_every_metadata_path_names_a_schema_node() {
    let schema = export();
    let bad: Vec<String> = meta()
        .keys
        .keys()
        .filter(|p| ui::node_at(&schema, p).is_none())
        .map(|p| {
            format!(
                "schema_ui.toml line {}: {p} names no node of the case schema",
                line_of(p)
            )
        })
        .collect();
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    let all = ui::schema_paths(&schema);
    let lacking = all.iter().filter(|p| !meta().keys.contains_key(*p)).count();
    eprintln!(
        "T3: {} entries in schema_ui.toml, all in the schema; {lacking} of {} schema paths have no entry",
        meta().keys.len(),
        all.len()
    );
}

/// T4: every recommended range lies inside the node's hard range, only on numeric nodes, and names its source.
#[test]
fn t4_recommended_ranges_are_inside_the_hard_range_and_cited() {
    let schema = export();
    let mut n = 0;
    for (p, m) in &meta().keys {
        let Some(r) = &m.recommended else { continue };
        n += 1;
        let node = ui::node_at(&schema, p).unwrap();
        let at = format!("schema_ui.toml line {}: {p}", line_of(p));
        assert!(
            matches!(node["type"].as_str(), Some("number" | "integer")),
            "{at}: not numeric"
        );
        assert!(
            r.source.trim().len() > 20,
            "{at}: a range needs its source (author, title, year, or an ADR)"
        );
        assert!(r.minimum.is_some() || r.maximum.is_some(), "{at}: an empty range");
        if let (Some(lo), Some(hi)) = (r.minimum, r.maximum) {
            assert!(lo <= hi, "{at}: minimum above maximum");
        }
        for x in [r.minimum, r.maximum].into_iter().flatten() {
            if let Some(lo) = node["minimum"].as_f64() {
                assert!(x >= lo, "{at}: {x} below the hard minimum {lo}");
            }
            if let Some(hi) = node["maximum"].as_f64() {
                assert!(x <= hi, "{at}: {x} above the hard maximum {hi}");
            }
        }
    }
    eprintln!("T4: {n} recommended ranges, each with its source");
}

/// T5: every written help is one line of at most 160 characters; every `see` names a built-in docs page and one of its
/// headings; every unit goes to a node that has none; every advanced reason is one line.
#[test]
fn t5_help_lines_see_headings_units_and_reasons() {
    let schema = export();
    for (p, m) in &meta().keys {
        let at = format!("schema_ui.toml line {}: {p}", line_of(p));
        for (what, text) in [("help", &m.help), ("label", &m.label), ("advanced", &m.advanced)] {
            if let Some(t) = text {
                assert!(
                    !t.trim().is_empty() && !t.contains('\n'),
                    "{at}: {what} must be one line"
                );
                assert!(
                    t.chars().count() <= ui::HELP_MAX,
                    "{at}: {what} over {} characters",
                    ui::HELP_MAX
                );
            }
        }
        if let Some(l) = &m.label {
            assert!(l.chars().count() <= 40, "{at}: a label is a short name");
        }
        if let Some(s) = &m.see {
            assert!(
                ui::doc_section(s).is_some(),
                "{at}: see = {s} names no built-in page and heading"
            );
        }
        if m.unit.is_some() {
            let node = ui::node_at(&schema, p).unwrap().as_object().unwrap();
            assert!(!ui::has_unit(node), "{at}: the schema already gives this node a unit");
        }
    }
    assert!(matches!(meta().advanced_list.status.as_str(), "proposed" | "approved"));
}

/// The fallback: the export's help lines are all one line of at most 160 characters, wherever they came from.
#[test]
fn every_help_line_of_the_export_is_short() {
    let v = ui::case_schema_ui().unwrap();
    for p in ui::schema_paths(&v) {
        if let Some(h) = ui::node_at(&v, &p).and_then(|n| n["x-rcfd-help"].as_str()) {
            assert!(!h.contains('\n') && h.chars().count() <= ui::HELP_MAX, "{p}: {h}");
        }
    }
    assert_eq!(v["x-rcfd-ui"], ui::UI_PROTOCOL);
}

/// Help coverage (ADR 0730 Q4): reported, not gated.
#[test]
fn help_coverage_is_reported() {
    let v = ui::case_schema_ui().unwrap();
    let c = ui::help_coverage(&v);
    let advanced = meta().keys.values().filter(|m| m.advanced.is_some()).count();
    let ranges = meta().keys.values().filter(|m| m.recommended.is_some()).count();
    eprintln!(
        "help coverage: {} nodes ({} leaves): written {} ({} leaves), from the docs {}, from the schema text {}, none {}; \
         advanced list ({}): {advanced} keys; recommended ranges: {ranges}",
        c.nodes,
        c.leaves,
        c.written,
        c.written_leaves,
        c.docs,
        c.schema,
        c.none,
        meta().advanced_list.status
    );
    assert_eq!(c.written + c.docs + c.schema + c.none, c.nodes);
}

/// Every key the example projects set has written help and a label (the help a learner meets first).
#[test]
fn example_project_keys_have_written_help() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../example-projects");
    let schema = export();
    let mut missing = Vec::new();
    let mut seen = 0;
    for project in std::fs::read_dir(&root).unwrap().flatten() {
        let cases = project.path().join("cases");
        for f in std::fs::read_dir(&cases).into_iter().flatten().flatten() {
            if f.path().extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let text = std::fs::read_to_string(f.path()).unwrap();
            let doc: toml::Value = toml::from_str(&text).unwrap();
            let mut paths = Vec::new();
            collect(&schema, &serde_json::to_value(&doc).unwrap(), "", &mut paths);
            for p in paths {
                seen += 1;
                let m = meta().keys.get(&p);
                if m.and_then(|m| m.help.as_ref()).is_none() || m.and_then(|m| m.label.as_ref()).is_none() {
                    missing.push(format!("{p} ({})", f.path().display()));
                }
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(seen > 50, "the example projects were read");
    assert!(
        missing.is_empty(),
        "keys of the example projects without written help and label:\n{}",
        missing.join("\n")
    );
}

/// The schema patterns of the leaves a case value sets.
fn collect(node: &Value, v: &Value, at: &str, out: &mut Vec<String>) {
    let join = |k: &str| {
        if at.is_empty() {
            k.to_string()
        } else {
            format!("{at}.{k}")
        }
    };
    match v {
        Value::Object(m)
            if node
                .get("properties")
                .is_some_and(|p| p.as_object().is_some_and(|p| !p.is_empty())) =>
        {
            for (k, x) in m {
                if let Some(c) = node["properties"].get(k) {
                    collect(c, x, &join(k), out);
                }
            }
        }
        Value::Object(m) if node.get("x-rcfd-named-objects") == Some(&Value::Bool(true)) => {
            for x in m.values() {
                collect(&node["additionalProperties"], x, &join("*"), out);
            }
        }
        _ => out.push(at.to_string()),
    }
}

/// T7: `help.glossary` returns every row of `docs/glossary.md`; a changed table shape fails.
#[test]
fn t7_glossary_returns_every_row() {
    let md = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/glossary.md"))
        .unwrap();
    assert_eq!(
        md.replace("\r\n", "\n"),
        ui::GLOSSARY_MD.replace("\r\n", "\n"),
        "the built-in copy is the file"
    );
    let rows = md
        .lines()
        .skip_while(|l| l.trim() != ui::GLOSSARY_HEADER)
        .skip(2)
        .take_while(|l| l.starts_with('|'))
        .count();
    let v = ui::glossary_json().unwrap();
    let terms = v["terms"].as_array().unwrap();
    assert_eq!(terms.len(), rows);
    assert!(rows >= 20);
    for t in terms {
        for k in ["term", "meaning", "page"] {
            assert!(!t[k].as_str().unwrap().is_empty(), "{t}");
        }
    }
    assert!(ui::parse_glossary("| Term | Meaning |\n| --- | --- |\n| a | b |\n").is_err());
    assert!(ui::parse_glossary(&format!("{}\n| --- | --- | --- |\n| a | b |\n", ui::GLOSSARY_HEADER)).is_err());
}
