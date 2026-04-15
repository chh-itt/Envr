//! `er` — two-letter launcher for `envr` (expects `envr` beside this binary, e.g. same install dir).

fn main() -> ! {
    let code = run_envr();
    std::process::exit(code);
}

fn run_envr() -> i32 {
    let envr = resolve_envr_executable();
    // Forward the full parent environment so `ENVR_*` (e.g. `ENVR_RUNTIME_ROOT`) and PATH
    // match what the user set in their shell before invoking `er`.
    let status = std::process::Command::new(&envr)
        .envs(std::env::vars_os())
        .args(std::env::args_os().skip(1))
        .status();
    match status {
        Ok(s) => s.code().unwrap_or(if s.success() { 0 } else { 1 }),
        Err(e) => {
            eprintln!("er: failed to run {}: {e}", envr.display());
            127
        }
    }
}

fn resolve_envr_executable() -> std::path::PathBuf {
    #[cfg(windows)]
    const SIDE_BY_SIDE: &str = "envr.exe";
    #[cfg(not(windows))]
    const SIDE_BY_SIDE: &str = "envr";

    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let cand = dir.join(SIDE_BY_SIDE);
        if cand.is_file() {
            return cand;
        }
    }
    std::path::PathBuf::from("envr")
}
