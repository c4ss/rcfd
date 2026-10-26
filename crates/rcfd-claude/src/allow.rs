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
    Ok(())
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
        let rule = match s.find('(') {
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
        };
        rule.validate().ok().map(|_| rule)
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
}
