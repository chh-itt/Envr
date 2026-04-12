use clap::CommandFactory;
use clap_complete::{generate, Shell};

pub fn run(shell: Shell) -> i32 {
    let mut cmd = crate::cli::Cli::command();
    let bin = cmd.get_bin_name().unwrap_or("envr").to_string();
    let mut out = std::io::stdout();
    generate(shell, &mut cmd, bin, &mut out);
    0
}
