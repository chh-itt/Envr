use envr_domain::runtime::RuntimeKind;
#[cfg(windows)]
use envr_domain::runtime::runtime_windows_prereqs;
use std::process::ExitStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsLaunchCategory {
    MissingDependency,
    BadExeFormat,
    InitFailure,
    MissingRuntimeLoader,
    Unknown,
}

#[cfg(windows)]
fn classify_windows_os_error(code: i32) -> Option<WindowsLaunchCategory> {
    match code {
        126 => Some(WindowsLaunchCategory::MissingDependency),
        193 => Some(WindowsLaunchCategory::BadExeFormat),
        1114 => Some(WindowsLaunchCategory::InitFailure),
        _ => None,
    }
}

#[cfg(windows)]
fn classify_windows_exit_code(code: i32) -> Option<WindowsLaunchCategory> {
    match code as u32 {
        0xC0000135 => Some(WindowsLaunchCategory::MissingRuntimeLoader),
        0xC000007B => Some(WindowsLaunchCategory::BadExeFormat),
        0xC0000142 => Some(WindowsLaunchCategory::InitFailure),
        _ => None,
    }
}

#[cfg(windows)]
fn classify_windows_stderr(stderr: &str) -> Option<WindowsLaunchCategory> {
    let s = stderr.to_ascii_lowercase();
    if s.contains("vcruntime") || s.contains("api-ms-win-crt") || s.contains("was not found") {
        return Some(WindowsLaunchCategory::MissingDependency);
    }
    if s.contains("0xc000007b") || s.contains("bad exe format") {
        return Some(WindowsLaunchCategory::BadExeFormat);
    }
    if s.contains("0xc0000142") {
        return Some(WindowsLaunchCategory::InitFailure);
    }
    None
}

#[cfg(windows)]
fn remediation_for_category(category: WindowsLaunchCategory) -> &'static str {
    match category {
        WindowsLaunchCategory::MissingDependency | WindowsLaunchCategory::MissingRuntimeLoader => {
            "可能缺少 Microsoft Visual C++ Redistributable for Visual Studio 2015-2022。请优先安装 x64，必要时补充 x86。"
        }
        WindowsLaunchCategory::BadExeFormat => {
            "疑似 32/64 位架构不匹配或依赖链不兼容。请检查是否安装了对应架构的运行时及 VC++ 运行库。"
        }
        WindowsLaunchCategory::InitFailure => {
            "进程初始化失败。建议先安装/修复 VC++ 运行库，并检查安全软件拦截。"
        }
        WindowsLaunchCategory::Unknown => "请检查系统依赖与 PATH，并重试。",
    }
}

pub fn classify_spawn_failure_message(
    kind: Option<RuntimeKind>,
    context: &str,
    err: &std::io::Error,
) -> String {
    #[cfg(windows)]
    {
        let category = err
            .raw_os_error()
            .and_then(classify_windows_os_error)
            .unwrap_or(WindowsLaunchCategory::Unknown);
        let mut msg = format!("{context} failed to start: {err}");
        if category != WindowsLaunchCategory::Unknown {
            msg.push_str("; ");
            msg.push_str(remediation_for_category(category));
        }
        if let Some(kind) = kind {
            let prereqs = runtime_windows_prereqs(kind);
            if !prereqs.is_empty() {
                let labels = prereqs
                    .iter()
                    .map(|p| p.as_label())
                    .collect::<Vec<_>>()
                    .join(", ");
                msg.push_str(" 推荐依赖: ");
                msg.push_str(&labels);
            }
        }
        msg
    }
    #[cfg(not(windows))]
    {
        let _ = kind;
        format!("{context} failed to start: {err}")
    }
}

pub fn classify_exit_failure_message(
    kind: Option<RuntimeKind>,
    context: &str,
    status: ExitStatus,
    stderr: &str,
) -> Option<String> {
    #[cfg(windows)]
    {
        let code = status.code()?;
        let category =
            classify_windows_exit_code(code).or_else(|| classify_windows_stderr(stderr))?;
        let mut msg = format!(
            "{context} exited with Windows failure code 0x{code:08X}; {}",
            remediation_for_category(category)
        );
        if let Some(kind) = kind {
            let prereqs = runtime_windows_prereqs(kind);
            if !prereqs.is_empty() {
                let labels = prereqs
                    .iter()
                    .map(|p| p.as_label())
                    .collect::<Vec<_>>()
                    .join(", ");
                msg.push_str(" 推荐依赖: ");
                msg.push_str(&labels);
            }
        }
        Some(msg)
    }
    #[cfg(not(windows))]
    {
        let _ = (kind, context, status, stderr);
        None
    }
}
