//! Monitor time series (residuals, forces, probes) with a documented JSON
//! format, CSV export and a self-contained HTML report (F-074).
//!
//! JSON format (version 1, [`monitor_json_schema`] returns its JSON Schema):
//!
//! ```json
//! { "format": "rcfd-monitors", "version": 1, "x_label": "iteration",
//!   "series": { "res_u": { "unit": "", "x": [1,2], "y": [1.0, 0.5] } } }
//! ```

use rcfd_core::{Error, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// One monitored quantity versus iteration or time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Series {
    /// Unit label (free text, may be empty).
    pub unit: String,
    /// Abscissa values (iteration number or time).
    pub x: Vec<f64>,
    /// Ordinate values.
    pub y: Vec<f64>,
}

/// Named collection of series sharing an abscissa meaning.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorStore {
    /// Meaning of `x` (`iteration` or `time`).
    pub x_label: String,
    series: BTreeMap<String, Series>,
}

impl MonitorStore {
    /// Empty store.
    pub fn new(x_label: &str) -> Self {
        MonitorStore {
            x_label: x_label.into(),
            series: BTreeMap::new(),
        }
    }

    /// Append a sample; creates the series on first use.
    pub fn push(&mut self, name: &str, unit: &str, x: f64, y: f64) {
        let s = self.series.entry(name.to_string()).or_default();
        if s.x.is_empty() {
            s.unit = unit.into();
        }
        s.x.push(x);
        s.y.push(y);
    }

    /// Series by name.
    pub fn get(&self, name: &str) -> Option<&Series> {
        self.series.get(name)
    }

    /// Series names (sorted).
    pub fn names(&self) -> Vec<&str> {
        self.series.keys().map(String::as_str).collect()
    }

    /// Last value of a series.
    pub fn last(&self, name: &str) -> Option<f64> {
        self.series.get(name).and_then(|s| s.y.last().copied())
    }

    /// Samples with `x > after` (incremental polling by clients).
    pub fn since(&self, name: &str, after: f64) -> Option<Series> {
        let s = self.series.get(name)?;
        let start = s.x.partition_point(|&v| v <= after);
        Some(Series {
            unit: s.unit.clone(),
            x: s.x[start..].to_vec(),
            y: s.y[start..].to_vec(),
        })
    }

    /// Serialise to the documented JSON format.
    pub fn to_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        for (k, s) in &self.series {
            m.insert(k.clone(), json!({"unit": s.unit, "x": s.x, "y": s.y}));
        }
        json!({"format": "rcfd-monitors", "version": 1, "x_label": self.x_label, "series": m})
    }

    /// Parse the documented JSON format.
    pub fn from_json(v: &Value) -> Result<Self> {
        let bad = |m: &str| Error::Io(format!("monitor json: {m}"));
        if v["format"] != "rcfd-monitors" || v["version"] != 1 {
            return Err(bad("not an rcfd-monitors version 1 document"));
        }
        let mut st = MonitorStore::new(v["x_label"].as_str().ok_or_else(|| bad("x_label"))?);
        for (k, s) in v["series"].as_object().ok_or_else(|| bad("series"))? {
            let arr = |key: &str| -> Result<Vec<f64>> {
                s[key]
                    .as_array()
                    .ok_or_else(|| bad(key))?
                    .iter()
                    .map(|x| x.as_f64().ok_or_else(|| bad(key)))
                    .collect()
            };
            let (x, y) = (arr("x")?, arr("y")?);
            if x.len() != y.len() {
                return Err(bad("x and y differ in length"));
            }
            st.series.insert(
                k.clone(),
                Series {
                    unit: s["unit"].as_str().unwrap_or("").into(),
                    x,
                    y,
                },
            );
        }
        Ok(st)
    }

    /// CSV with one row per distinct abscissa (`x_label,name1,name2,...`);
    /// missing samples are left empty.
    pub fn to_csv(&self) -> String {
        let names = self.names();
        let mut xs: Vec<f64> = self.series.values().flat_map(|s| s.x.iter().copied()).collect();
        xs.sort_by(f64::total_cmp);
        xs.dedup();
        let mut out = format!("{},{}\n", self.x_label, names.join(","));
        for x in xs {
            out.push_str(&format!("{x}"));
            for n in &names {
                let s = &self.series[*n];
                match s.x.binary_search_by(|v| v.total_cmp(&x)) {
                    Ok(i) => out.push_str(&format!(",{:e}", s.y[i])),
                    Err(_) => out.push(','),
                }
            }
            out.push('\n');
        }
        out
    }
}

/// JSON Schema of the monitor JSON format.
pub fn monitor_json_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "rcfd monitor data",
        "type": "object",
        "required": ["format", "version", "x_label", "series"],
        "additionalProperties": false,
        "properties": {
            "format": {"type": "string", "enum": ["rcfd-monitors"]},
            "version": {"type": "integer", "minimum": 1, "maximum": 1},
            "x_label": {"type": "string", "enum": ["iteration", "time"]},
            "series": {
                "type": "object",
                "additionalProperties": {
                    "type": "object",
                    "required": ["unit", "x", "y"],
                    "additionalProperties": false,
                    "properties": {
                        "unit": {"type": "string"},
                        "x": {"type": "array", "items": {"type": "number"}},
                        "y": {"type": "array", "items": {"type": "number"}}
                    }
                }
            }
        }
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Inline SVG line plot of the named series (log-y when `log_y`).
pub fn plot_svg(store: &MonitorStore, names: &[&str], log_y: bool, title: &str) -> String {
    const COLORS: [&str; 6] = ["#2a6fdb", "#d9480f", "#2b8a3e", "#862e9c", "#c2255c", "#495057"];
    let (w, h, ml, mr, mt, mb) = (640.0, 300.0, 64.0, 12.0, 28.0, 36.0);
    let tr = |y: f64| if log_y { y.abs().max(1e-300).log10() } else { y };
    let mut pts = Vec::new();
    for n in names {
        if let Some(s) = store.get(n) {
            for (x, y) in s.x.iter().zip(&s.y) {
                if y.is_finite() {
                    pts.push((*x, tr(*y)));
                }
            }
        }
    }
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"{}\">",
        esc(title)
    );
    svg.push_str(&format!(
        "<text x=\"{ml}\" y=\"18\" font-size=\"13\" fill=\"currentColor\">{}</text>",
        esc(title)
    ));
    if pts.is_empty() {
        svg.push_str("<text x=\"70\" y=\"150\" fill=\"currentColor\">no data</text></svg>");
        return svg;
    }
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in &pts {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    if x1 - x0 < 1e-300 {
        x1 = x0 + 1.0;
    }
    if y1 - y0 < 1e-300 {
        y0 -= 0.5;
        y1 += 0.5;
    }
    let sx = |x: f64| ml + (x - x0) / (x1 - x0) * (w - ml - mr);
    let sy = |y: f64| h - mb - (y - y0) / (y1 - y0) * (h - mt - mb);
    svg.push_str(&format!(
        "<rect x=\"{ml}\" y=\"{mt}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"currentColor\" opacity=\"0.3\"/>",
        w - ml - mr,
        h - mt - mb
    ));
    let fmt = |v: f64| {
        if log_y {
            format!("1e{v:.0}")
        } else {
            format!("{v:.3e}")
        }
    };
    svg.push_str(&format!(
        "<text x=\"4\" y=\"{}\" font-size=\"10\" fill=\"currentColor\">{}</text><text x=\"4\" y=\"{}\" font-size=\"10\" fill=\"currentColor\">{}</text>",
        mt + 8.0,
        fmt(y1),
        h - mb,
        fmt(y0)
    ));
    svg.push_str(&format!(
        "<text x=\"{ml}\" y=\"{}\" font-size=\"10\" fill=\"currentColor\">{x0}</text><text x=\"{}\" y=\"{}\" font-size=\"10\" text-anchor=\"end\" fill=\"currentColor\">{x1} ({})</text>",
        h - 6.0,
        w - mr,
        h - 6.0,
        esc(&store.x_label)
    ));
    for (i, n) in names.iter().enumerate() {
        let Some(s) = store.get(n) else { continue };
        let color = COLORS[i % COLORS.len()];
        let path: Vec<String> =
            s.x.iter()
                .zip(&s.y)
                .filter(|(_, y)| y.is_finite())
                .map(|(x, y)| format!("{:.1},{:.1}", sx(*x), sy(tr(*y))))
                .collect();
        svg.push_str(&format!(
            "<polyline fill=\"none\" stroke=\"{color}\" stroke-width=\"1.5\" points=\"{}\"/>",
            path.join(" ")
        ));
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"{color}\">{}</text>",
            ml + 8.0 + 90.0 * i as f64,
            mt + 14.0,
            esc(n)
        ));
    }
    svg.push_str("</svg>");
    svg
}

/// One row of the integrals table in a report.
#[derive(Debug, Clone)]
pub struct ReportValue {
    /// Report name.
    pub name: String,
    /// Value.
    pub value: f64,
    /// Unit label.
    pub unit: String,
}

/// Self-contained HTML report with residual plots, monitor plots and the
/// table of integral values.
pub fn html_report(title: &str, summary: &[(String, String)], store: &MonitorStore, values: &[ReportValue]) -> String {
    let residuals: Vec<&str> = store.names().into_iter().filter(|n| n.starts_with("res_")).collect();
    let others: Vec<&str> = store.names().into_iter().filter(|n| !n.starts_with("res_")).collect();
    let mut h = String::new();
    h.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">");
    h.push_str(&format!("<title>{}</title>", esc(title)));
    h.push_str("<style>:root{color-scheme:light dark}body{font:14px/1.5 system-ui,sans-serif;max-width:760px;margin:2rem auto;padding:0 16px}table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #8884;padding:4px 8px;text-align:left}svg{width:100%;height:auto;margin:8px 0}h2{margin-top:2rem}</style></head><body>");
    h.push_str(&format!("<h1>{}</h1>", esc(title)));
    if !summary.is_empty() {
        h.push_str("<h2>Summary</h2><table>");
        for (k, v) in summary {
            h.push_str(&format!("<tr><th>{}</th><td>{}</td></tr>", esc(k), esc(v)));
        }
        h.push_str("</table>");
    }
    h.push_str("<h2>Residuals</h2>");
    h.push_str(&plot_svg(store, &residuals, true, "Normalised residuals"));
    if !values.is_empty() {
        h.push_str("<h2>Integrals</h2><table><tr><th>Report</th><th>Value</th><th>Unit</th></tr>");
        for v in values {
            h.push_str(&format!(
                "<tr><td>{}</td><td>{:.6e}</td><td>{}</td></tr>",
                esc(&v.name),
                v.value,
                esc(&v.unit)
            ));
        }
        h.push_str("</table>");
    }
    for n in others {
        h.push_str(&plot_svg(store, &[n], false, n));
    }
    h.push_str("</body></html>");
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_core::settings::validate_json_schema;

    fn store() -> MonitorStore {
        let mut s = MonitorStore::new("iteration");
        for i in 1..=20 {
            s.push("res_u", "", i as f64, 10f64.powf(-0.2 * i as f64));
            s.push("res_p", "", i as f64, 10f64.powf(-0.3 * i as f64));
            if i % 2 == 0 {
                s.push("cd", "", i as f64, 1.0 / i as f64);
            }
        }
        s
    }

    #[test]
    fn json_round_trip_and_schema_validation() {
        let s = store();
        let j = s.to_json();
        validate_json_schema(&monitor_json_schema(), &j).unwrap();
        assert_eq!(MonitorStore::from_json(&j).unwrap(), s);
        let mut bad = j.clone();
        bad["version"] = json!(2);
        assert!(validate_json_schema(&monitor_json_schema(), &bad).is_err());
        assert!(MonitorStore::from_json(&bad).is_err());
    }

    #[test]
    fn incremental_polling_and_csv() {
        let s = store();
        let t = s.since("res_u", 17.0).unwrap();
        assert_eq!(t.x, vec![18.0, 19.0, 20.0]);
        assert!(s.since("nope", 0.0).is_none());
        let csv = s.to_csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "iteration,cd,res_p,res_u");
        assert_eq!(lines.len(), 21);
        assert!(lines[1].starts_with("1,,"), "{}", lines[1]);
        assert!(lines[2].starts_with("2,5e-1,"), "{}", lines[2]);
        assert_eq!(s.last("cd"), Some(0.05));
    }

    #[test]
    fn html_report_contains_residual_plots_and_integrals() {
        let vals = vec![ReportValue {
            name: "drag".into(),
            value: 1.25,
            unit: "N".into(),
        }];
        let html = html_report("Cavity <Re=100>", &[("Cells".into(), "100".into())], &store(), &vals);
        assert!(html.matches("<polyline").count() >= 3); // 2 residuals + cd
        assert!(html.contains("Cavity &lt;Re=100&gt;"));
        assert!(html.contains("drag") && html.contains("1.250000e0"));
        assert!(html.contains("Normalised residuals"));
        assert!(plot_svg(&MonitorStore::new("time"), &["x"], false, "t").contains("no data"));
    }
}
