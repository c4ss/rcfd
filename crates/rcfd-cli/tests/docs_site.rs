//! F-096 (ADR 0089): the documentation site. Every Markdown file under docs/ becomes a static HTML page; the build fails on a
//! relative link that does not resolve to a generated page. The generator is this file (no external tool or crate): it knows
//! headings, fenced code, paragraphs, lists, pipe tables, inline code, bold and links, nothing more.
//!
//! Build: `cargo test -p rcfd-cli --test docs_site -- --nocapture` (prints the output directory).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `x.md#a` -> `x.html#a` for relative links; external and anchor-only links unchanged.
fn rewrite(url: &str) -> String {
    if url.contains("://") || url.starts_with('#') || url.starts_with("mailto:") {
        return url.to_string();
    }
    let (p, frag) = url.split_once('#').map_or((url, None), |(p, f)| (p, Some(f)));
    let p = p.strip_suffix(".md").map_or(p.to_string(), |s| format!("{s}.html"));
    frag.map_or(p.clone(), |f| format!("{p}#{f}"))
}

/// Inline markup: `code`, **bold**, [text](url); everything else escaped.
fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('`') {
            if let Some(end) = r.find('`') {
                out.push_str(&format!("<code>{}</code>", esc(&r[..end])));
                rest = &r[end + 1..];
                continue;
            }
        }
        if let Some(r) = rest.strip_prefix("**") {
            if let Some(end) = r.find("**") {
                out.push_str(&format!("<strong>{}</strong>", inline(&r[..end])));
                rest = &r[end + 2..];
                continue;
            }
        }
        if let Some(r) = rest.strip_prefix('[') {
            if let Some((text, after)) = r.split_once("](") {
                if let Some(end) = after.find(')') {
                    if !text.contains(']') {
                        let url = rewrite(&after[..end]);
                        out.push_str(&format!("<a href=\"{}\">{}</a>", esc(&url), inline(text)));
                        rest = &after[end + 1..];
                        continue;
                    }
                }
            }
        }
        let c = rest.chars().next().unwrap();
        out.push_str(&esc(&c.to_string()));
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn cells(row: &str) -> Vec<&str> {
    let r = row.trim().trim_start_matches('|').trim_end_matches('|');
    r.split('|').map(str::trim).collect()
}

/// Markdown to an HTML body; also returns the first heading as the title.
fn md_to_html(md: &str) -> (String, String) {
    let mut html = String::new();
    let mut title = String::new();
    let mut para: Vec<&str> = Vec::new();
    let mut list: Option<&str> = None; // "ul" or "ol"
    let lines: Vec<&str> = md.lines().collect();
    let flush = |html: &mut String, para: &mut Vec<&str>, list: &mut Option<&str>| {
        if !para.is_empty() {
            html.push_str(&format!("<p>{}</p>\n", inline(&para.join(" "))));
            para.clear();
        }
        if let Some(l) = list.take() {
            html.push_str(&format!("</li></{l}>\n"));
        }
    };
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        let t = l.trim_start();
        if let Some(info) = t.strip_prefix("```") {
            flush(&mut html, &mut para, &mut list);
            let lang = info.split_whitespace().next().unwrap_or("");
            let mut code = String::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("```") {
                code.push_str(lines[i]);
                code.push('\n');
                i += 1;
            }
            html.push_str(&format!(
                "<pre class=\"{}\"><code>{}</code></pre>\n",
                esc(lang),
                esc(&code)
            ));
        } else if t.is_empty() {
            flush(&mut html, &mut para, &mut list);
        } else if l.starts_with('#') && l.trim_start_matches('#').starts_with(' ') {
            flush(&mut html, &mut para, &mut list);
            let n = l.len() - l.trim_start_matches('#').len();
            let text = l[n..].trim();
            if title.is_empty() {
                title = text.replace('`', "");
            }
            let id: String = text
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
                .map(|c| if c == ' ' { '-' } else { c })
                .collect();
            html.push_str(&format!("<h{n} id=\"{id}\">{}</h{n}>\n", inline(text)));
        } else if t.starts_with('|') {
            flush(&mut html, &mut para, &mut list);
            html.push_str("<table>\n");
            let mut first = true;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                let row = cells(lines[i]);
                let sep = row
                    .iter()
                    .all(|c| !c.is_empty() && c.chars().all(|x| matches!(x, '-' | ':' | ' ')));
                if !sep {
                    let tag = if first { "th" } else { "td" };
                    let r: String = row.iter().map(|c| format!("<{tag}>{}</{tag}>", inline(c))).collect();
                    html.push_str(&format!("<tr>{r}</tr>\n"));
                }
                first = false;
                i += 1;
            }
            html.push_str("</table>\n");
            continue;
        } else if let Some(item) = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .map(|s| ("ul", s))
            .or_else(|| {
                let d = t.len() - t.trim_start_matches(|c: char| c.is_ascii_digit()).len();
                (d > 0).then(|| t[d..].strip_prefix(". ")).flatten().map(|s| ("ol", s))
            })
        {
            if !para.is_empty() || list.is_some_and(|k| k != item.0) {
                flush(&mut html, &mut para, &mut list);
            }
            match list {
                Some(_) => html.push_str("</li>\n<li>"),
                None => html.push_str(&format!("<{}>\n<li>", item.0)),
            }
            list = Some(item.0);
            html.push_str(&inline(item.1));
        } else if list.is_some() {
            html.push(' ');
            html.push_str(&inline(t));
        } else {
            para.push(t);
        }
        i += 1;
    }
    flush(&mut html, &mut para, &mut list);
    (title, html)
}

const STYLE: &str = "body{font-family:system-ui,sans-serif;max-width:60rem;margin:0 auto;padding:1rem;line-height:1.5;color:#1b1b1b;background:#fff}\
pre{background:#f3f3f3;padding:.6rem;overflow-x:auto}code{font-size:.92em}table{border-collapse:collapse}\
td,th{border:1px solid #ccc;padding:.2rem .4rem;vertical-align:top}nav{margin-bottom:1rem}";

fn md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            md_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
}

/// Builds the site into `out`; returns the generated pages (relative paths) and every relative link as (page, target).
fn build(docs: &Path, out: &Path) -> (BTreeSet<String>, Vec<(String, String)>) {
    let _ = std::fs::remove_dir_all(out);
    let mut files = Vec::new();
    md_files(docs, &mut files);
    let mut pages = BTreeSet::new();
    let mut links = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(docs).unwrap().with_extension("html");
        let rel_s = rel.to_string_lossy().replace('\\', "/");
        let depth = rel_s.matches('/').count();
        let (title, body) = md_to_html(&std::fs::read_to_string(f).unwrap());
        let home = format!("{}index.html", "../".repeat(depth));
        let page = format!(
            "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{} - rcfd</title><style>{STYLE}</style></head>\n<body><nav><a href=\"{home}\">rcfd documentation</a></nav>\n{body}</body></html>\n",
            esc(&title)
        );
        for href in page.split("href=\"").skip(1).filter_map(|s| s.split('"').next()) {
            if !href.contains("://") && !href.starts_with('#') && !href.starts_with("mailto:") {
                links.push((rel_s.clone(), href.replace("&amp;", "&")));
            }
        }
        let dest = out.join(&rel);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(&dest, page).unwrap();
        pages.insert(rel_s);
    }
    (pages, links)
}

/// Resolve `href` relative to `page` (both '/'-separated); None if it climbs above the site root.
fn resolve(page: &str, href: &str) -> Option<String> {
    let mut parts: Vec<&str> = page.split('/').collect();
    parts.pop();
    for c in href.split('#').next().unwrap().split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
}

/// S1, S2, S3.
#[test]
fn docs_site_builds_with_no_broken_links() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("docs-site");
    let (pages, links) = build(&docs, &out);
    let mut n_md = Vec::new();
    md_files(&docs, &mut n_md);
    println!(
        "docs site: {} pages, {} relative links, written to {}",
        pages.len(),
        links.len(),
        out.display()
    );
    assert_eq!(pages.len(), n_md.len(), "S1");
    assert!(pages.len() >= 100, "S1: {} pages", pages.len());
    let broken: Vec<_> = links
        .iter()
        .filter(|(p, h)| resolve(p, h).map_or(true, |t| !pages.contains(&t)))
        .collect();
    assert!(broken.is_empty(), "S2: broken links {broken:?}");
    let index: BTreeSet<String> = links
        .iter()
        .filter(|(p, _)| p == "index.html")
        .filter_map(|(p, h)| resolve(p, h))
        .collect();
    let mut want: Vec<String> = ["case-schema", "meshing", "milestone-fan", "design-loop", "mcp-tools"]
        .iter()
        .map(|s| format!("{s}.html"))
        .collect();
    for e in std::fs::read_dir(docs.join("tutorials")).unwrap() {
        let p = e.unwrap().path();
        want.push(format!("tutorials/{}.html", p.file_stem().unwrap().to_string_lossy()));
    }
    for w in want {
        assert!(index.contains(&w), "S3: index.html does not link {w}");
    }
}

#[test]
fn markdown_subset() {
    let (title, h) = md_to_html(
        "# T `x`\n\npara **b** [l](a.md#s) <x>\n\n- one\n  more\n- two\n\n1. a\n\n| h | k |\n|---|---|\n| 1 | `2` |\n\n```toml run\na = 1 < 2\n```\n",
    );
    assert_eq!(title, "T x");
    for want in [
        "<h1 id=\"t-x\">T <code>x</code></h1>",
        "<p>para <strong>b</strong> <a href=\"a.html#s\">l</a> &lt;x&gt;</p>",
        "<ul>\n<li>one more</li>\n<li>two</li></ul>",
        "<ol>\n<li>a</li></ol>",
        "<tr><th>h</th><th>k</th></tr>\n<tr><td>1</td><td><code>2</code></td></tr>",
        "<pre class=\"toml\"><code>a = 1 &lt; 2\n</code></pre>",
    ] {
        assert!(h.contains(want), "{want}\nnot in\n{h}");
    }
    assert_eq!(resolve("tutorials/a.html", "../b.html#x").as_deref(), Some("b.html"));
    assert_eq!(resolve("a.html", "../b.html"), None);
}
