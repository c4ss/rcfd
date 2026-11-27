//! `rcfd-solid-run`: the solid driver the runner launches for a run of a solid case file (F-990, ADR 0840 D2), with the
//! arguments it gives `rcfd run`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(rcfd_workspace::solid_run::main_args(&args));
}
