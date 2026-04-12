use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io::{self, Write};

pub fn run(shell: Shell) -> i32 {
    let mut cmd = crate::cli::Cli::command();
    let bin = cmd.get_bin_name().unwrap_or("envr").to_string();
    let mut buf = Vec::<u8>::new();
    generate(shell, &mut cmd, bin, &mut buf);
    let header = "# envr: built-in argv shorthands run before clap — see `envr help shortcuts`\n";
    let mut out = io::stdout().lock();
    let _ = out.write_all(header.as_bytes());
    let _ = out.write_all(&buf);
    0
}
