//! The conservative allow-list (user decision 2026-10-26, ADR 0126): its rules, their flag values and the per-project
//! allow-list file.
//!
//! The bridge of the removed egui Studio ran every session with `--permission-mode dontAsk --permission-prompts none`: a tool call that no rule allows is
//! denied, never prompted and never auto-approved. The allow-list is the only thing the app adds. Observed on 2.1.310
//! and stated in ADR 0126: the CLI also approves some read-only shell commands by its own classification
//! (`echo bye` was accepted without a rule), user and project settings add their own rules, and an `Edit(path)` rule
//! also permits `Write` to that path. The allow-list is therefore a floor the app controls, not the whole gate.

use crate::Error;
use std::path::Path;

/// One allow-list rule. Shell and file rules always carry a pattern; a bare `Bash` or `Edit` cannot be expressed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AllowRule {
    /// A whole tool that takes no pattern, e.g. `Grep`, `Glob` or an MCP tool `mcp__rcfd__list_runs`.
    Tool(String),
    /// `Bash(<command pattern>)`; the command name must be literal.
    Bash(String),
    /// `PowerShell(<command pattern>)`; the command name must be literal.
    PowerShell(String),
    /// `Edit(./<project-relative path pattern>)`; also covers `Write` (observed).
    Edit(String),
    /// `Read(./<project-relative path pattern>)`.
    Read(String),
}

/// Tools whose bare name would allow every use of them; they need a pattern.
const NEED_PATTERN: [&str; 8] = [
    "Bash",
    "PowerShell",
    "Edit",
    "Write",
    "MultiEdit",
    "Read",
    "NotebookEdit",
    "WebFetch",
];

fn check_text(s: &str) -> Result<(), &'static str> {
    if s.is_empty() {
        return Err("empty pattern");
    }
    if s.trim() != s {
        return Err("leading or trailing whitespace");
    }
    if s.chars().any(|c| c.is_control() || matches!(c, '(' | ')' | ',')) {
        return Err("contains '(', ')', ',' or a control character");
    }
    if s.starts_with('-') {
        return Err("starts with '-'");
    }
    let low = s.to_ascii_lowercase();
    if low.contains("dangerously") || low.contains("bypasspermissions") {
        return Err("contains a permission-bypass token");
    }
    Ok(())
}

fn check_command(s: &str) -> Result<(), &'static str> {
    check_text(s)?;
    let first = s.split_whitespace().next().unwrap_or("");
    if first.contains(['*', '?']) {
        return Err("the command name must be literal (no wildcard-only shell rule)");
    }
    Ok(())
}

fn check_path(s: &str) -> Result<(), &'static str> {
    check_text(s)?;
    let p = strip_dot(s);
    if p.starts_with(['/', '~']) || p.contains(['\\', ':']) {
        return Err("path must be project-relative with '/' separators");
    }
    if p.split('/').any(|seg| seg == "..") {
        return Err("path must not leave the project folder");
    }
    if !p.chars().any(|c| c.is_alphanumeric()) {
        return Err("pattern must name something (no wildcard-only file rule)");
    }
    if p.contains('~') {
        return Err("'~' is refused (a short 8.3 name can reach a control file)");
    }
    let segs = p.split('/').filter(|s| !s.is_empty() && *s != ".");
    if segs.enumerate().any(|(i, s)| control_segment(s, i == 0)) {
        return Err("path reaches a file that configures Claude or the project (.mcp.json, CLAUDE.md, .claude, .rcfd, rcfd-project.json)");
    }
    Ok(())
}

/// Names no rule may reach, lowercase (ADR 0128 finding 2).
const CONTROL: [&str; 5] = [".mcp.json", "claude.md", ".claude", ".rcfd", "rcfd-project.json"];

/// Does one path segment name a control file? ASCII case and trailing dots and spaces are ignored (Windows does).
/// A wildcard segment counts when it is the first one and could match a control name.
// shortcut: a wildcard in a deeper segment (`cases/**`) can still match a nested CLAUDE.md; refusing it would remove
// the main use of the list (ADR 0128 known limit).
fn control_segment(seg: &str, first: bool) -> bool {
    let s = seg.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    if s.contains(['*', '?', '[']) {
        return first && CONTROL.iter().any(|c| glob(s.as_bytes(), c.as_bytes()));
    }
    CONTROL.contains(&s.as_str())
}

/// Glob match of one segment: `*` any run, `?` one character, `[...]` any one character (wider than a class).
fn glob(p: &[u8], s: &[u8]) -> bool {
    match p.split_first() {
        None => s.is_empty(),
        Some((b'*', rest)) => (0..=s.len()).any(|i| glob(rest, &s[i..])),
        Some((b'?', rest)) => !s.is_empty() && glob(rest, &s[1..]),
        Some((b'[', rest)) => match rest.iter().position(|&c| c == b']') {
            Some(k) => !s.is_empty() && glob(&rest[k + 1..], &s[1..]),
            None => s.first() == Some(&b'[') && glob(rest, &s[1..]),
        },
        Some((c, rest)) => s.first() == Some(c) && glob(rest, &s[1..]),
    }
}

fn strip_dot(s: &str) -> &str {
    s.trim_start_matches("./")
}

impl AllowRule {
    /// Checks the rule against the conservative policy.
    pub fn validate(&self) -> Result<(), Error> {
        let r = match self {
            AllowRule::Tool(n) => {
                let mut cs = n.chars();
                if !cs.next().is_some_and(|c| c.is_ascii_alphabetic())
                    || !cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    Err("tool name must be letters, digits and '_'")
                } else if NEED_PATTERN.contains(&n.as_str()) || check_text(n).is_err() {
                    Err("this tool needs a pattern, e.g. Bash(cmd) or Edit(path)")
                } else {
                    Ok(())
                }
            }
            AllowRule::Bash(p) | AllowRule::PowerShell(p) => check_command(p),
            AllowRule::Edit(p) | AllowRule::Read(p) => check_path(p),
        };
        r.map_err(|reason| Error::InvalidRule {
            rule: self.to_flag_value(),
            reason,
        })
    }

    /// The `--allowedTools` value for this rule.
    pub fn to_flag_value(&self) -> String {
        match self {
            AllowRule::Tool(n) => n.clone(),
            AllowRule::Bash(p) => format!("Bash({p})"),
            AllowRule::PowerShell(p) => format!("PowerShell({p})"),
            AllowRule::Edit(p) => format!("Edit(./{})", strip_dot(p)),
            AllowRule::Read(p) => format!("Read(./{})", strip_dot(p)),
        }
    }

    /// Parses a flag value back into a rule (the inverse of [`AllowRule::to_flag_value`]); `None` if not rule-shaped.
    pub fn parse(s: &str) -> Option<AllowRule> {
        let rule = Self::parse_shape(s)?;
        rule.validate().ok().map(|_| rule)
    }

    /// The rule a flag value has the shape of, not yet validated.
    fn parse_shape(s: &str) -> Option<AllowRule> {
        Some(match s.find('(') {
            None => AllowRule::Tool(s.to_string()),
            Some(open) => {
                let inner = s.get(open + 1..)?.strip_suffix(')')?.to_string();
                match &s[..open] {
                    "Bash" => AllowRule::Bash(inner),
                    "PowerShell" => AllowRule::PowerShell(inner),
                    "Edit" => AllowRule::Edit(inner),
                    "Read" => AllowRule::Read(inner),
                    _ => return None,
                }
            }
        })
    }

    /// The same rule with the `./` anchor of a file pattern removed, so a rule read back
    /// from its flag value equals the rule that was written.
    fn normalized(self) -> AllowRule {
        match self {
            AllowRule::Edit(p) => AllowRule::Edit(strip_dot(&p).to_string()),
            AllowRule::Read(p) => AllowRule::Read(strip_dot(&p).to_string()),
            r => r,
        }
    }
}

/// A validated set of rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AllowList {
    rules: Vec<AllowRule>,
}

impl AllowList {
    /// An empty list: every tool that needs approval is denied.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a rule after validating it; duplicates are ignored.
    pub fn add(&mut self, rule: AllowRule) -> Result<(), Error> {
        rule.validate()?;
        if !self.rules.contains(&rule) {
            self.rules.push(rule);
        }
        Ok(())
    }

    /// The rules in insertion order.
    pub fn rules(&self) -> &[AllowRule] {
        &self.rules
    }
}

/// Where the per-project allow-list file lives, relative to the project folder (ADR 0134 W48-11 (a)). `.rcfd/` is a
/// control path that no rule can reach, so Claude cannot edit the file through the list.
pub const ALLOW_FILE: &str = ".rcfd/claude-allow.json";

/// Name of the one MCP server in the app-owned config file (ADR 0134 W48-10 (a)). It has no `-`, so the tool names
/// `mcp__rcfd_project__<tool>` pass [`AllowRule::Tool`] validation unchanged.
pub const PINNED_SERVER: &str = "rcfd_project";

/// The read-class project tools: the first contents of a new project's allow-list, under the app-owned server's names
/// (ADR 0134 W48-11 (a)). The 11 of ADR 0135's tool table, then the read tools added since (ADR 0187 Q6 (b) and ADR 0188
/// Q6 (b), taken by ADR 0195 P5). An existing allow-list file is not changed by this list.
pub const PROJECT_READ_TOOLS: [&str; 15] = [
    "project_info",
    "list_runs",
    "run_status",
    "read_metrics",
    "read_residuals",
    "field_summary",
    "render_field",
    "compare_runs",
    "list_changes",
    "read_project_file",
    "wait_run",
    "read_feed",
    "check_case_mesh",
    "preview_mesh",
    "list_meshes",
];

/// Format version of the allow-list file.
const FILE_VERSION: u64 = 1;

/// The file's shape: `{"version": 1, "rules": ["<flag value>", ...]}`, nothing else (each rule is validated after the
/// parse and located by [`rule_positions`]).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FileIn {
    #[allow(dead_code)] // checked while it is read
    version: FileVersion,
    rules: Vec<String>,
}

#[derive(serde::Serialize)]
struct FileOut<'a> {
    version: u64,
    rules: Vec<&'a str>,
}

struct FileVersion;

impl<'de> serde::Deserialize<'de> for FileVersion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = u64::deserialize(d)?;
        if v != FILE_VERSION {
            return Err(serde::de::Error::custom(format!(
                "version {v} is not {FILE_VERSION}, the only version this build reads"
            )));
        }
        Ok(FileVersion)
    }
}

/// Line and column (1-based) of every string element of the top-level `"rules"` array of `text`, in order.
/// `text` has already been parsed as JSON, so the scan only follows nesting and strings. serde_json's position of an
/// error raised for one element lies after the next separator, possibly on a later line, so it is not used for rules.
fn rule_positions(text: &str) -> Vec<(usize, usize)> {
    let b = text.as_bytes();
    let (mut i, mut line, mut line_start, mut depth) = (0usize, 1usize, 0usize, 0usize);
    let (mut expect_key, mut last_key, mut in_rules) = (false, None::<String>, false);
    let mut out = Vec::new();
    while i < b.len() {
        match b[i] {
            b'\n' => {
                line += 1;
                line_start = i + 1;
            }
            b'{' => {
                depth += 1;
                expect_key = depth == 1;
            }
            b'}' => depth = depth.saturating_sub(1),
            b'[' => {
                depth += 1;
                in_rules = depth == 2 && last_key.as_deref() == Some("rules");
            }
            b']' => {
                if depth == 2 {
                    in_rules = false;
                }
                depth = depth.saturating_sub(1);
            }
            b',' if depth == 1 => expect_key = true,
            b':' if depth == 1 => expect_key = false,
            b'"' => {
                let (start, col) = (i, i - line_start + 1);
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                if depth == 1 && expect_key {
                    last_key = text.get(start..=i).and_then(|t| serde_json::from_str(t).ok());
                } else if depth == 2 && in_rules {
                    out.push((line, col));
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

impl AllowList {
    /// The first allow-list of a project: the read-class project tools ([`PROJECT_READ_TOOLS`]) as
    /// `mcp__rcfd_project__<tool>`, nothing else (ADR 0134 W48-11 (a)).
    pub fn project_initial() -> Self {
        let mut l = Self::new();
        for t in PROJECT_READ_TOOLS {
            l.add(AllowRule::Tool(format!("mcp__{PINNED_SERVER}__{t}")))
                .expect("project tool names are valid rules");
        }
        l
    }

    /// The allow-list file of `project` ([`ALLOW_FILE`]).
    pub fn file_of(project: &Path) -> std::path::PathBuf {
        project.join(ALLOW_FILE)
    }

    /// The file contents: pretty JSON, one rule (its flag value) per line, ending in a newline.
    pub fn to_file_bytes(&self) -> Vec<u8> {
        let values: Vec<String> = self.rules.iter().map(AllowRule::to_flag_value).collect();
        let out = FileOut {
            version: FILE_VERSION,
            rules: values.iter().map(String::as_str).collect(),
        };
        let mut v = serde_json::to_vec_pretty(&out).expect("strings serialize");
        v.push(b'\n');
        v
    }

    /// Reads a list from file contents. Every rule is re-validated by the same policy as [`AllowList::add`]; one
    /// invalid rule, an unknown key, another version or malformed JSON refuses the whole file with its line and
    /// column ([`Error::AllowFile`]), so nothing from a tampered file is passed. `path` only names the file in errors.
    pub fn from_file_bytes(path: &Path, bytes: &[u8]) -> Result<Self, Error> {
        let file: FileIn = serde_json::from_slice(bytes).map_err(|e| {
            let (line, column) = (e.line(), e.column());
            let text = e.to_string();
            let suffix = format!(" at line {line} column {column}");
            Error::AllowFile {
                path: path.to_path_buf(),
                line,
                column,
                reason: text.strip_suffix(&suffix).unwrap_or(&text).to_string(),
            }
        })?;
        let refuse = |k: usize, reason: String| {
            let text = std::str::from_utf8(bytes).unwrap_or("");
            let (line, column) = rule_positions(text).get(k).copied().unwrap_or((0, 0));
            Error::AllowFile {
                path: path.to_path_buf(),
                line,
                column,
                reason,
            }
        };
        let mut l = Self::new();
        for (k, s) in file.rules.iter().enumerate() {
            let rule = AllowRule::parse_shape(s).ok_or_else(|| {
                refuse(
                    k,
                    format!("{s:?} is not a rule (a tool name, Bash(..), PowerShell(..), Edit(..) or Read(..))"),
                )
            })?;
            rule.validate().map_err(|e| refuse(k, e.to_string()))?;
            l.add(rule.normalized()).map_err(|e| refuse(k, e.to_string()))?; // duplicates collapse
        }
        Ok(l)
    }

    /// Reads the allow-list file at `path` (see [`AllowList::from_file_bytes`]). A missing file is an
    /// [`Error::Io`] with kind `NotFound`.
    pub fn from_file(path: &Path) -> Result<Self, Error> {
        let bytes = rcfd_project::atomic::read(path).map_err(|e| project_err(path, e))?;
        Self::from_file_bytes(path, &bytes)
    }

    /// Replaces the allow-list file at `path` atomically (`rcfd_project::atomic`), creating its folder.
    pub fn to_file(&self, path: &Path) -> Result<(), Error> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|source| Error::Io {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        rcfd_project::atomic::atomic_write(path, &self.to_file_bytes()).map_err(|e| project_err(path, e))
    }
}

/// An `rcfd-project` file error as a bridge error, keeping the I/O error (and so its kind) where there is one.
pub(crate) fn project_err(path: &Path, e: rcfd_project::ProjectError) -> Error {
    use rcfd_project::ProjectError as P;
    match e {
        P::Io { path, source, .. } | P::Retry { path, source, .. } => Error::Io { path, source },
        other => Error::Io {
            path: path.to_path_buf(),
            source: std::io::Error::other(other.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Registered table T1 (ADR 0126): rule -> exact flag value.
    fn table() -> Vec<(AllowRule, &'static str)> {
        vec![
            (AllowRule::Tool("Grep".into()), "Grep"),
            (AllowRule::Tool("mcp__rcfd__list_runs".into()), "mcp__rcfd__list_runs"),
            (AllowRule::Bash("echo hello".into()), "Bash(echo hello)"),
            (AllowRule::Bash("git status".into()), "Bash(git status)"),
            (AllowRule::Bash("cargo test *".into()), "Bash(cargo test *)"),
            (
                AllowRule::PowerShell("Get-ChildItem cases".into()),
                "PowerShell(Get-ChildItem cases)",
            ),
            (AllowRule::Edit("note.txt".into()), "Edit(./note.txt)"),
            (AllowRule::Edit("./cases/**".into()), "Edit(./cases/**)"),
            (
                AllowRule::Read("runs/*/metrics.json".into()),
                "Read(./runs/*/metrics.json)",
            ),
        ]
    }

    fn mismatches(map: fn(&AllowRule) -> String) -> usize {
        table().iter().filter(|(r, want)| map(r) != *want).count()
    }

    #[test]
    fn t1_mapping_is_exact_and_a_wrong_mapping_is_caught() {
        assert_eq!(mismatches(AllowRule::to_flag_value), 0);
        // negative control: Edit rendered without the './' anchor (relative to an unknown base) must be caught
        fn wrong(r: &AllowRule) -> String {
            match r {
                AllowRule::Edit(p) => format!("Edit({p})"),
                o => o.to_flag_value(),
            }
        }
        assert_eq!(mismatches(wrong), 1); // Edit("note.txt"); Edit("./cases/**") already carries the anchor
        let mut l = AllowList::new();
        l.add(AllowRule::Bash("echo hello".into())).unwrap();
        l.add(AllowRule::Edit("note.txt".into())).unwrap();
        l.add(AllowRule::Edit("note.txt".into())).unwrap(); // duplicate ignored
        assert_eq!(
            l.rules().iter().map(AllowRule::to_flag_value).collect::<Vec<_>>(),
            ["Bash(echo hello)", "Edit(./note.txt)"]
        );
        for (r, s) in table() {
            assert_eq!(
                AllowRule::parse(s).map(|p| p.to_flag_value()),
                Some(r.to_flag_value()),
                "round trip {s}"
            );
        }
    }

    #[test]
    fn t2_policy_rejects_broad_or_injecting_rules() {
        let bad = [
            AllowRule::Tool("Bash".into()),
            AllowRule::Tool("Edit".into()),
            AllowRule::Tool("Write".into()),
            AllowRule::Tool("Read".into()),
            AllowRule::Tool("*".into()),
            AllowRule::Tool("--dangerously-skip-permissions".into()),
            AllowRule::Bash("*".into()),
            AllowRule::Bash("* --help".into()),
            AllowRule::Bash("".into()),
            AllowRule::Bash("echo a),Bash(*".into()),
            AllowRule::Bash("echo a\nrm -rf x".into()),
            AllowRule::Bash(" echo".into()),
            AllowRule::Bash("-rf".into()),
            AllowRule::Edit("**".into()),
            AllowRule::Edit("./**/*".into()),
            AllowRule::Edit("/etc/passwd".into()),
            AllowRule::Edit("//c/x".into()),
            AllowRule::Edit("~/.ssh/x".into()),
            AllowRule::Edit("C:/x".into()),
            AllowRule::Edit("cases\\a.json".into()),
            AllowRule::Edit("../outside.txt".into()),
            AllowRule::Read("cases/../../x".into()),
        ];
        for r in bad {
            assert!(r.validate().is_err(), "accepted {r:?}");
            assert!(AllowList::new().add(r).is_err());
        }
    }

    /// T5 (ADR 0128 finding 2): rules never reach the files that configure Claude or the project. (Its part on
    /// suggestions went with `suggest`, F-838.)
    #[test]
    fn t5_control_files_are_refused() {
        let names = [".mcp.json", "CLAUDE.md", ".claude", ".rcfd", "rcfd-project.json"];
        let mut forms = Vec::new();
        for n in names {
            for x in [n.to_string(), n.to_ascii_uppercase(), n.to_ascii_lowercase()] {
                forms.extend([
                    x.clone(),
                    format!("./{x}"),
                    format!("{x}."),
                    format!("cases/../{x}"),
                    format!("a\\{x}"),
                    format!("cases/{x}"),
                ]);
            }
        }
        for d in [".claude", ".rcfd", ".Claude", ".RCFD"] {
            forms.extend([
                format!("{d}/settings.json"),
                format!("{d}/**"),
                format!("./{d}/sub/f"),
                format!("{d}./x"),
            ]);
        }
        forms.extend(
            [
                "*.md",
                ".c*",
                "*.json",
                "rcfd-*",
                "**/CLAUDE.md",
                "CLAUDE~1.MD",
                ".//CLAUDE.md",
            ]
            .map(String::from),
        );
        let accepted: Vec<&String> = forms
            .iter()
            .filter(|f| {
                AllowRule::Edit((*f).clone()).validate().is_ok() || AllowRule::Read((*f).clone()).validate().is_ok()
            })
            .collect();
        assert!(accepted.is_empty(), "control-file rules accepted: {accepted:?}");
        // ordinary paths stay allowed
        for ok in [
            "cases/x.json",
            "notes/a.md",
            "./cases/**",
            "runs/*/metrics.json",
            "note.txt",
        ] {
            assert!(AllowRule::Edit(ok.into()).validate().is_ok(), "{ok}");
            assert!(AllowRule::Read(ok.into()).validate().is_ok(), "{ok}");
        }
        eprintln!("T5: {} control-file forms refused", forms.len());
    }
}
