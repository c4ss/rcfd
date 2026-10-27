//! `rcfd-runner`: the per-project run service (`serve`) and a command-line client of it (ADR 0123).

use rcfd_runner::client::Client;
use rcfd_runner::records::RunSettings;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "rcfd-runner - run service of an rcfd project folder

USAGE:
    rcfd-runner <command> [--project <dir>] [options]     (default project: the current directory)

COMMANDS:
    serve                      run the service in the foreground (exit 3 if one already runs for the project)
    ensure                     start the service, detached, unless it runs
    start <case> [--priority n] [--threads n] [--mem-mb n] [--set path=value]... [--no-runner]
                               queue a run (and start the service unless --no-runner); prints the run id
    list                       every run and its status (JSON lines)
    status <id>                status.json of a run
    pause <id> | resume <id> | cancel <id>
    wait <id> [--timeout seconds]
                               wait until the run is finished, failed or cancelled (exit 1 unless finished)
    shutdown                   stop the service cleanly; solvers keep running and the next service adopts them
";

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn num<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> Result<T, String> {
    opt(args, name).map_or(Ok(default), |v| {
        v.parse().map_err(|_| format!("{name} needs a number, got '{v}'"))
    })
}

fn run(args: &[String]) -> Result<u8, String> {
    let Some(cmd) = args.first().map(String::as_str) else {
        print!("{USAGE}");
        return Ok(0);
    };
    let rest = &args[1..];
    let project = PathBuf::from(opt(rest, "--project").unwrap_or("."));
    let mut pos = Vec::new();
    let mut skip = false;
    for a in rest {
        if skip {
            skip = false;
        } else if a.starts_with("--") {
            skip = !matches!(a.as_str(), "--no-runner");
        } else {
            pos.push(a.as_str());
        }
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let client = Client::new(&project, exe);
    let id = || pos.first().copied().ok_or_else(|| format!("{cmd} needs a run id"));
    match cmd {
        "-h" | "--help" | "help" => print!("{USAGE}"),
        "serve" => return rcfd_runner::runner::serve(&project).map(|c| c as u8),
        "ensure" => client.ensure_runner()?,
        "start" => {
            let case = pos.first().ok_or("start needs a case file")?;
            let settings = RunSettings {
                priority: num(rest, "--priority", 0)?,
                threads: num(rest, "--threads", 1)?,
                mem_mb: opt(rest, "--mem-mb")
                    .map(|v| v.parse())
                    .transpose()
                    .map_err(|_| "--mem-mb needs a number")?,
                set: rest
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| *a == "--set")
                    .filter_map(|(i, _)| rest.get(i + 1).cloned())
                    .collect(),
            };
            let id = if rest.iter().any(|a| a == "--no-runner") {
                client.submit(case.as_ref(), &settings)?
            } else {
                client.start(case.as_ref(), &settings)?
            };
            println!("{id}");
        }
        "list" => {
            for (id, st) in client.list()? {
                println!("{}", serde_json::json!({"id": id, "status": st}));
            }
        }
        "status" => println!(
            "{}",
            serde_json::to_string_pretty(&client.status(id()?)?).unwrap_or_default()
        ),
        "pause" => client.pause(id()?)?,
        "resume" => client.resume(id()?)?,
        "cancel" => client.cancel(id()?)?,
        "wait" => {
            let secs = num(rest, "--timeout", 86_400.0)?;
            let timeout = Duration::try_from_secs_f64(secs).map_err(|e| {
                let raw = opt(rest, "--timeout").unwrap_or_default();
                format!("--timeout needs a non-negative number of seconds, got '{raw}' ({e})")
            })?;
            let st = client.wait(id()?, timeout)?;
            println!("{}", serde_json::to_string_pretty(&st).unwrap_or_default());
            return Ok(u8::from(st.state != rcfd_runner::records::RunState::Finished));
        }
        "shutdown" => client.shutdown(Duration::from_secs(30))?,
        other => return Err(format!("unknown command '{other}' (see rcfd-runner --help)")),
    }
    Ok(0)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(c) => ExitCode::from(c),
        Err(e) => {
            eprintln!("rcfd-runner: {e}");
            ExitCode::from(1)
        }
    }
}
