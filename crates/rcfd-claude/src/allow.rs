//! The conservative allow-list (user decision 2026-10-26, ADR 0126) and its mapping onto `claude -p` flags.
//!
//! Every session runs with `--permission-mode dontAsk --permission-prompts none`: a tool call that no rule allows is
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
/// Flags the bridge never passes: they change who approves or widen access beyond the project folder.
const FORBIDDEN_FLAGS: [&str; 4] = [
    "--settings",
    "--permission-prompt-tool",
    "--add-dir",
    "--allow-dangerously-skip-permissions",
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

    /// The same rule with the `./` anchor of a file pattern removed (the form [`suggest`] gives), so a rule read back
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

    /// Removes a rule.
    pub fn remove(&mut self, rule: &AllowRule) {
        self.rules.retain(|r| r != rule);
    }

    /// The rules in insertion order.
    pub fn rules(&self) -> &[AllowRule] {
        &self.rules
    }

    /// The permission flags for `claude -p`: always `dontAsk` with prompts off, then the rules (one argument each).
    pub fn permission_args(&self) -> Vec<String> {
        let mut a: Vec<String> = ["--permission-mode", "dontAsk", "--permission-prompts", "none"]
            .map(String::from)
            .to_vec();
        if !self.rules.is_empty() {
            a.push("--allowedTools".into());
            a.extend(self.rules.iter().map(AllowRule::to_flag_value));
        }
        a
    }
}

/// Independent check of a complete argument list before spawn: refuses any bypass or auto-approve flag, any
/// permission mode other than `dontAsk`, prompts other than `none`, and any `--allowedTools` value that is not a valid rule.
pub fn check_args(args: &[String]) -> Result<(), Error> {
    let bad = |a: &str| Err(Error::ForbiddenArgument(a.to_string()));
    let next = |i: usize| args.get(i + 1).map(String::as_str);
    let (mut modes, mut prompts, mut in_tools) = (0, 0, false);
    for (i, a) in args.iter().enumerate() {
        let low = a.to_ascii_lowercase();
        if low.contains("dangerously") || low.contains("bypasspermissions") {
            return bad(a);
        }
        if FORBIDDEN_FLAGS
            .iter()
            .any(|f| low == *f || low.starts_with(&format!("{f}=")))
        {
            return bad(a);
        }
        if low.starts_with("--allowedtools=") || low.starts_with("--allowed-tools=") {
            return bad(a); // the CLI splits such values on commas; only one-rule-per-argument is passed
        }
        if a.starts_with('-') {
            in_tools = false;
        } else if in_tools && AllowRule::parse(a).is_none() {
            return bad(a);
        }
        if low.starts_with("--permission-mode") {
            modes += 1;
            if a != "--permission-mode" || next(i) != Some("dontAsk") {
                return bad(a);
            }
        }
        if low.starts_with("--permission-prompts") {
            prompts += 1;
            if a != "--permission-prompts" || next(i) != Some("none") {
                return bad(a);
            }
        }
        if a == "--allowedTools" || a == "--allowed-tools" {
            in_tools = true;
        }
    }
    if modes != 1 || prompts != 1 {
        return bad("exactly one '--permission-mode dontAsk' and one '--permission-prompts none' are required");
    }
    Ok(())
}

/// Where the per-project allow-list file lives, relative to the project folder (ADR 0134 W48-11 (a)). `.rcfd/` is a
/// control path that no rule can reach, so Claude cannot edit the file through the list.
pub const ALLOW_FILE: &str = ".rcfd/claude-allow.json";

/// Name of the one MCP server in the app-owned config file (ADR 0134 W48-10 (a)). It has no `-`, so the tool names
/// `mcp__rcfd_project__<tool>` pass [`AllowRule::Tool`] validation unchanged.
pub const PINNED_SERVER: &str = "rcfd_project";

/// The read-class project tools of ADR 0135 (access `read` in its tool table): the first contents of a project's
/// allow-list, under the app-owned server's names (ADR 0134 W48-11 (a)).
pub const PROJECT_READ_TOOLS: [&str; 11] = [
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
    /// The first allow-list of a project: the read-class project tools of ADR 0135 as `mcp__rcfd_project__<tool>`,
    /// nothing else (ADR 0134 W48-11 (a)).
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

/// The app-owned MCP config for `--mcp-config` (ADR 0134 W48-10 (a)): one stdio server named [`PINNED_SERVER`] that
/// starts `rcfd_mcp --project <project> --root <project>` (ADR 0135 W48-13 (a)). The Studio writes it into its own
/// config folder, never into the project.
pub fn pinned_mcp_config(rcfd_mcp: &Path, project: &Path) -> String {
    let p = project.to_string_lossy();
    let v = serde_json::json!({
        "mcpServers": {
            PINNED_SERVER: {
                "type": "stdio",
                "command": rcfd_mcp.to_string_lossy(),
                "args": ["--project", p, "--root", p],
            }
        }
    });
    serde_json::to_string_pretty(&v).expect("strings serialize") + "\n"
}

/// [`check_args`] plus the MCP pinning rules of ADR 0134 W48-10 (a) for a chat whose working folder is `project`.
/// `--strict-mcp-config` and `--mcp-config` are accepted only as separate arguments, each at most once, and
/// `--mcp-config` only together with `--strict-mcp-config` and followed by exactly one value: an absolute path of an
/// existing file outside `project`, lexically and after resolving links and short names. Inline JSON, a relative path
/// (it would resolve inside the project) and the `=` forms are refused. Argument lists without these flags get exactly
/// the verdict of [`check_args`].
pub fn check_args_in(args: &[String], project: &Path) -> Result<(), Error> {
    check_args(args)?;
    let bad = |a: &str| Err(Error::ForbiddenArgument(a.to_string()));
    let (mut strict, mut configs) = (0, 0);
    for (i, a) in args.iter().enumerate() {
        let low = a.to_ascii_lowercase();
        if low.starts_with("--strict-mcp-config") {
            if a != "--strict-mcp-config" {
                return bad(a);
            }
            strict += 1;
        } else if low.starts_with("--mcp-config") {
            if a != "--mcp-config" {
                return bad(a);
            }
            configs += 1;
            let Some(value) = args.get(i + 1) else {
                return bad("--mcp-config without a file");
            };
            if let Some(next) = args.get(i + 2).filter(|n| !n.starts_with('-')) {
                return bad(next); // the option takes several values; exactly one file is passed
            }
            if let Err(why) = mcp_config_outside(value, project) {
                return Err(Error::ForbiddenArgument(format!("--mcp-config {value}: {why}")));
            }
        }
    }
    if strict > 1 || configs > 1 {
        return bad("--strict-mcp-config and --mcp-config may each appear only once");
    }
    if configs == 1 && strict == 0 {
        return bad("--mcp-config without --strict-mcp-config");
    }
    Ok(())
}

/// Is `value` an absolute path of an existing file outside `project`?
fn mcp_config_outside(value: &str, project: &Path) -> Result<(), String> {
    let t = value.trim_start();
    if t.is_empty() || t.starts_with(['-', '{', '[']) {
        return Err("a config file path is required (no inline JSON, no flag)".into());
    }
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err("the path must be absolute (a relative one resolves inside the project folder)".into());
    }
    if inside(&lexical(project), &lexical(path)) {
        return Err("the file is inside the project folder".into());
    }
    let canon = |p: &Path| rcfd_project::paths::canonicalize(p).map_err(|e| format!("{}: {e}", p.display()));
    let (cp, cf) = (canon(project)?, canon(path)?);
    if !std::fs::metadata(&cf).is_ok_and(|m| m.is_file()) {
        return Err("not a file".into());
    }
    if inside(&lexical(&cp), &lexical(&cf)) {
        return Err("the file resolves inside the project folder".into());
    }
    Ok(())
}

/// The components of `p` with `.` dropped and `..` applied, as strings (ASCII-lowercased on Windows).
fn lexical(p: &Path) -> Vec<String> {
    use std::path::Component;
    let (mut out, mut roots): (Vec<String>, usize) = (Vec::new(), 0);
    for c in p.components() {
        match c {
            Component::CurDir => continue,
            Component::ParentDir => {
                if out.len() > roots {
                    out.pop(); // never above the root
                }
                continue;
            }
            Component::Prefix(_) | Component::RootDir => roots += 1,
            Component::Normal(_) => {}
        }
        let s = c.as_os_str().to_string_lossy().replace('\\', "/");
        out.push(if cfg!(windows) { s.to_ascii_lowercase() } else { s });
    }
    out
}

/// Is `p` equal to `base` or below it (whole components, so `proj2` is not below `proj`)?
fn inside(base: &[String], p: &[String]) -> bool {
    p.len() >= base.len() && p[..base.len()] == *base
}

/// The narrowest rule that would allow a denied call, for an "allow this" button. `None` when no safe exact rule
/// exists (a file outside the project, a command with wildcards or several lines). Accepting it only edits the list.
pub fn suggest(tool_name: &str, input: Option<&serde_json::Value>, project: &Path) -> Option<AllowRule> {
    let field = |k: &str| input.and_then(|v| v.get(k)).and_then(|v| v.as_str());
    let rule = match tool_name {
        "Bash" | "PowerShell" => {
            let cmd = field("command")?;
            if cmd.contains(['*', '?']) {
                return None; // a wildcard in a suggestion would widen it beyond the denied call
            }
            if tool_name == "Bash" {
                AllowRule::Bash(cmd.to_string())
            } else {
                AllowRule::PowerShell(cmd.to_string())
            }
        }
        "Write" | "Edit" | "MultiEdit" | "NotebookEdit" | "Read" => {
            let rel = project_relative(project, field("file_path").or_else(|| field("notebook_path"))?)?;
            if rel.contains(['*', '?', '[', ']', '!']) {
                return None;
            }
            if tool_name == "Read" {
                AllowRule::Read(rel)
            } else {
                AllowRule::Edit(rel)
            }
        }
        other => AllowRule::Tool(other.to_string()),
    };
    rule.validate().ok().map(|_| rule)
}

/// `file` relative to `project` with '/' separators, or `None` when it is outside (ASCII case-insensitive on Windows).
pub fn project_relative(project: &Path, file: &str) -> Option<String> {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_string();
    let f = norm(file);
    if !Path::new(file).is_absolute() && !f.starts_with('/') {
        return Some(f);
    }
    let p = norm(&project.to_string_lossy());
    let (pc, fc) = if cfg!(windows) {
        (p.to_ascii_lowercase(), f.to_ascii_lowercase())
    } else {
        (p.clone(), f.clone())
    };
    fc.strip_prefix(&format!("{pc}/")).map(|_| f[p.len() + 1..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
                                          // and the full argument list is exact
        let mut l = AllowList::new();
        l.add(AllowRule::Bash("echo hello".into())).unwrap();
        l.add(AllowRule::Edit("note.txt".into())).unwrap();
        l.add(AllowRule::Edit("note.txt".into())).unwrap(); // duplicate ignored
        assert_eq!(
            l.permission_args(),
            [
                "--permission-mode",
                "dontAsk",
                "--permission-prompts",
                "none",
                "--allowedTools",
                "Bash(echo hello)",
                "Edit(./note.txt)"
            ]
        );
        assert_eq!(
            AllowList::new().permission_args(),
            ["--permission-mode", "dontAsk", "--permission-prompts", "none"]
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

    #[test]
    fn t3_forbidden_flags_never_generated_and_checker_discriminates() {
        // generated space: every subset (up to 4 rules) of candidates built from adversarial fragments; invalid ones are
        // rejected by `add`, valid ones must give an argument list that passes the independent checker and carries
        // no forbidden token
        let frags = [
            "echo hi",
            "bypassPermissions",
            "--dangerously-skip-permissions",
            "auto",
            "acceptEdits",
            "*",
            "a,b",
            "x)",
            "note.txt",
            "cases/**",
            "..",
            "-p",
            "git status",
            "Grep",
        ];
        let mut cands = Vec::new();
        for f in frags {
            for r in [
                AllowRule::Tool(f.into()),
                AllowRule::Bash(f.into()),
                AllowRule::PowerShell(f.into()),
                AllowRule::Edit(f.into()),
                AllowRule::Read(f.into()),
            ] {
                cands.push(r);
            }
        }
        let n = cands.len();
        let (mut lists, mut accepted_rules) = (0usize, 0usize);
        for a in 0..n {
            for b in (a..n).step_by(3) {
                for c in (b..n).step_by(7) {
                    for d in (c..n).step_by(11) {
                        let mut l = AllowList::new();
                        for &i in &[a, b, c, d] {
                            if l.add(cands[i].clone()).is_ok() {
                                accepted_rules += 1;
                            }
                        }
                        let args = l.permission_args();
                        check_args(&args).unwrap_or_else(|e| panic!("{e}: {args:?}"));
                        for x in &args {
                            let low = x.to_ascii_lowercase();
                            assert!(
                                !low.contains("dangerously") && !low.contains("bypasspermissions"),
                                "{x}"
                            );
                        }
                        assert_eq!(
                            args[..4],
                            ["--permission-mode", "dontAsk", "--permission-prompts", "none"]
                        );
                        assert_eq!(args.iter().filter(|x| x.starts_with("--permission-mode")).count(), 1);
                        lists += 1;
                    }
                }
            }
        }
        assert!(
            lists >= 5000 && accepted_rules >= 10_000,
            "space too small: {lists} lists, {accepted_rules} rules"
        );
        // negative controls: hand-built bad argument lists are refused
        let base = AllowList::new().permission_args();
        let with = |extra: &[&str]| {
            let mut v = base.clone();
            v.extend(extra.iter().map(|s| s.to_string()));
            v
        };
        let bad_lists = vec![
            with(&["--dangerously-skip-permissions"]),
            with(&["--allow-dangerously-skip-permissions"]),
            with(&["--permission-mode", "bypassPermissions"]),
            ["--permission-mode", "auto", "--permission-prompts", "none"]
                .map(String::from)
                .to_vec(),
            ["--permission-mode", "acceptEdits", "--permission-prompts", "none"]
                .map(String::from)
                .to_vec(),
            ["--permission-mode=dontAsk", "--permission-prompts", "none"]
                .map(String::from)
                .to_vec(),
            ["--permission-mode", "dontAsk", "--permission-prompts", "host"]
                .map(String::from)
                .to_vec(),
            ["--permission-prompts", "none"].map(String::from).to_vec(),
            with(&["--settings", "{}"]),
            with(&["--permission-prompt-tool", "mcp__x__y"]),
            with(&["--add-dir", "C:/"]),
            with(&["--allowedTools", "Bash"]),
            with(&["--allowedTools", "Edit(**)"]),
            with(&["--allowedTools", "Grep", "Bash(*)"]),
            with(&["--allowedTools=Grep,Bash"]),
        ];
        for b in bad_lists {
            assert!(check_args(&b).is_err(), "checker accepted {b:?}");
        }
    }

    #[test]
    fn t4_suggestions_are_exact_and_stay_in_the_project() {
        let proj = if cfg!(windows) {
            Path::new("C:\\proj\\demo")
        } else {
            Path::new("/proj/demo")
        };
        let inside = if cfg!(windows) {
            "c:\\PROJ\\demo\\cases\\a.json"
        } else {
            "/proj/demo/cases/a.json"
        };
        let outside = if cfg!(windows) {
            "C:\\proj\\demo2\\a.json"
        } else {
            "/proj/demo2/a.json"
        };
        assert_eq!(
            suggest("Write", Some(&json!({"file_path": inside})), proj),
            Some(AllowRule::Edit("cases/a.json".into()))
        );
        assert_eq!(
            suggest("Read", Some(&json!({"file_path": inside})), proj),
            Some(AllowRule::Read("cases/a.json".into()))
        );
        assert_eq!(suggest("Write", Some(&json!({"file_path": outside})), proj), None);
        assert_eq!(
            suggest("Bash", Some(&json!({"command": "echo hello > x.txt"})), proj),
            Some(AllowRule::Bash("echo hello > x.txt".into()))
        );
        assert_eq!(suggest("Bash", Some(&json!({"command": "rm *"})), proj), None);
        assert_eq!(suggest("Bash", Some(&json!({"command": "a\nb"})), proj), None);
        assert_eq!(suggest("Bash", None, proj), None);
        assert_eq!(suggest("WebFetch", None, proj), None);
        assert_eq!(
            suggest("mcp__rcfd__start_run", None, proj),
            Some(AllowRule::Tool("mcp__rcfd__start_run".into()))
        );
    }

    /// T5 (ADR 0128 finding 2): rules and suggestions never reach the files that configure Claude or the project.
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
        let (proj, sep) = if cfg!(windows) {
            (Path::new("C:\\proj\\demo"), "\\")
        } else {
            (Path::new("/proj/demo"), "/")
        };
        for f in [
            ".mcp.json",
            "CLAUDE.md",
            ".claude/settings.json",
            ".rcfd/runner-config.json",
            "rcfd-project.json",
        ] {
            let mut abs = format!("{}{sep}{}", proj.display(), f.replace('/', sep));
            for tool in ["Write", "Edit", "Read"] {
                assert_eq!(
                    suggest(tool, Some(&json!({"file_path": abs})), proj),
                    None,
                    "{tool} {abs}"
                );
            }
            if cfg!(windows) {
                abs = abs.to_ascii_uppercase();
                assert_eq!(suggest("Write", Some(&json!({"file_path": abs})), proj), None, "{abs}");
            }
        }
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
        assert_eq!(
            suggest(
                "Write",
                Some(&json!({"file_path": format!("{}{sep}notes{sep}a.md", proj.display())})),
                proj
            ),
            Some(AllowRule::Edit("notes/a.md".into()))
        );
        eprintln!("T5: {} control-file forms refused", forms.len());
    }
}
