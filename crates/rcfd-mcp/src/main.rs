//! `rcfd-mcp`: MCP server on stdio.
//!
//! Options: `--root <dir>` confine file access, `--read-only` refuse mutating
//! tools, `--max-cells <n>`, `--max-steps <n>`, `--max-run-seconds <s>`,
//! `--cases <dir>`.

use rcfd_api::service::{Service, ServiceConfig};
use rcfd_mcp::{policy::Policy, McpServer};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("rcfd-mcp: MCP server (stdio)\n\nOPTIONS:\n    --root <dir>            confine all file access to <dir>\n    --read-only             refuse tools that change state\n    --max-cells <n>         refuse larger meshes (default 2000000)\n    --max-steps <n>         largest iteration count per run call\n    --max-run-seconds <s>   wall-clock cap per run\n    --cases <dir>           cases directory for list_cases");
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
    cfg.max_run_seconds = val("--max-run-seconds").and_then(|v| v.parse().ok());
    let policy = if args.iter().any(|a| a == "--read-only") {
        Policy::read_only()
    } else {
        Policy::default()
    };
    let mut server = McpServer::new(Service::new(cfg), policy);
    if let Some(c) = val("--cases") {
        server = server.with_cases_dir(c);
    }
    match server.serve(std::io::stdin(), std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
