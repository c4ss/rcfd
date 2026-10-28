//! `rcfd-mcp`: MCP server on stdio.
//!
//! Options: `--root <dir>` confine file access, `--read-only` refuse mutating
//! tools, `--max-cells <n>`, `--max-steps <n>`, `--max-run-seconds <s>`,
//! `--cases <dir>`, `--project <dir>` add the project tools of ADR 0135 for that project folder.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::{policy::Policy, project::ProjectTools, McpServer};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("rcfd-mcp: MCP server (stdio)\n\nOPTIONS:\n    --root <dir>            confine all file access to <dir>\n    --read-only             refuse tools that change state\n    --max-cells <n>         refuse larger meshes (default 2000000)\n    --max-steps <n>         largest iteration count per run call\n    --max-run-seconds <s>   wall-clock cap per run\n    --max-design-evaluations <n>  case runs one design call may start (default 1000)\n    --cases <dir>           cases directory for list_cases\n    --project <dir>         add the project tools for this rcfd project folder (ADR 0135);\n                            without --root, file access is confined to it");
        return ExitCode::SUCCESS;
    }
    let val = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let mut cfg = ServiceConfig {
        root: val("--root").map(PathBuf::from),
        ..Default::default()
    };
    if let Some(n) = val("--max-cells").and_then(|v| v.parse().ok()) {
        cfg.max_cells = n;
    }
    if let Some(n) = val("--max-steps").and_then(|v| v.parse().ok()) {
        cfg.max_steps_per_call = n;
    }
    if let Some(n) = val("--max-design-evaluations").and_then(|v| v.parse().ok()) {
        cfg.max_design_evaluations = n;
    }
    cfg.max_run_seconds = val("--max-run-seconds").and_then(|v| v.parse().ok());
    let policy = if args.iter().any(|a| a == "--read-only") {
        Policy::read_only()
    } else {
        Policy::default()
    };
    // ADR 0135 W48-13 (a): the project comes from the argument, never from the working directory; a folder that is
    // not a project stops the server with a located message (a session outside a project has no project tools)
    let project = match args.iter().position(|a| a == "--project") {
        None => None,
        Some(i) => {
            let Some(dir) = args.get(i + 1) else {
                eprintln!("error: --project needs a folder");
                return ExitCode::from(2);
            };
            match ProjectTools::open(std::path::Path::new(dir)) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("error: --project {dir}: {e}");
                    return ExitCode::from(2);
                }
            }
        }
    };
    // ADR 0135 amendment 3 item 2: --project implies --root <project> when --root is not given
    if cfg.root.is_none() {
        cfg.root = project.as_ref().map(|p| p.root().to_path_buf());
    }
    let mut server = McpServer::new(Service::new(cfg), policy);
    if let Some(c) = val("--cases") {
        server = server.with_cases_dir(c);
    }
    if let Some(p) = project {
        server = server.with_project(p);
    }
    match server.serve(std::io::stdin(), std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
