use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::commands::doctor::{DoctorReport, all_kinds};
use crate::commands::doctor_analyzer::{current_is_broken, runtime_root_writable};
use crate::commands::shim_cmd;
use crate::output::fmt_template;

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::RuntimeVersion;

/// Compare version-like labels for picking a reasonable "latest" `current` in `--fix`.
fn cmp_version_labels(a: &str, b: &str) -> std::cmp::Ordering {
    fn tokens(s: &str) -> Vec<&str> {
        s.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| !t.is_empty())
            .collect()
    }
    let ta = tokens(a);
    let tb = tokens(b);
    let n = ta.len().max(tb.len());
    for i in 0..n {
        let va = ta.get(i).copied().unwrap_or("");
        let vb = tb.get(i).copied().unwrap_or("");
        let ord = match (va.parse::<u64>(), vb.parse::<u64>()) {
            (Ok(na), Ok(nb)) => na.cmp(&nb),
            _ => va.cmp(vb),
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

fn pick_latest_installed(installed: &[RuntimeVersion]) -> Option<RuntimeVersion> {
    installed
        .iter()
        .max_by(|x, y| cmp_version_labels(&x.0, &y.0))
        .cloned()
}

pub(crate) fn apply_doctor_fixes(
    g: &GlobalArgs,
    service: &RuntimeService,
    report: &DoctorReport,
) -> Vec<String> {
    let mut applied = Vec::new();
    let shims = report.root.join("shims");
    let empty_shims = shims.is_dir()
        && std::fs::read_dir(&shims)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true);

    if empty_shims && report.root.exists() && runtime_root_writable(&report.root) {
        common::emit_verbose_step(
            g,
            &envr_core::i18n::tr_key(
                "cli.verbose.doctor.fix_shims",
                "[verbose] 正在刷新 shims",
                "[verbose] refreshing shims",
            ),
        );
        match shim_cmd::sync_core_shims_strict(g) {
            Ok(kinds) => {
                applied.push(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.fix.shims_ok",
                        "已刷新核心 shims：{kinds}",
                        "refreshed core shims: {kinds}",
                    ),
                    &[("kinds", &kinds.join(", "))],
                ));
            }
            Err(e) => {
                applied.push(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.fix.shims_err",
                        "刷新 shims 失败：{detail}",
                        "failed to refresh shims: {detail}",
                    ),
                    &[("detail", &e.to_string())],
                ));
            }
        }
    }

    for kind in all_kinds() {
        let Ok(index) = service.index_port(kind) else {
            continue;
        };
        let Ok(installed) = index.list_installed() else {
            continue;
        };
        let Ok(current) = index.current() else {
            continue;
        };
        if installed.is_empty() {
            continue;
        }

        let was_broken = current_is_broken(&current, &installed);
        let need_set = current.is_none() || was_broken;
        if !need_set {
            continue;
        }
        let Some(best) = pick_latest_installed(&installed) else {
            continue;
        };
        common::emit_verbose_step(
            g,
            &fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.verbose.doctor.fix_current",
                    "[verbose] 正在修复 current：{kind} -> {version}",
                    "[verbose] fixing current: {kind} -> {version}",
                ),
                &[("kind", kind_label(kind)), ("version", &best.0)],
            ),
        );
        let Ok(installer) = service.installer_port(kind) else {
            continue;
        };
        if let Err(e) = installer.set_current(&best) {
            applied.push(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.doctor.fix.current_err",
                    "{kind}：无法设置 current：{detail}",
                    "{kind}: could not set current: {detail}",
                ),
                &[("kind", kind_label(kind)), ("detail", &e.to_string())],
            ));
            continue;
        }
        let tmpl = if was_broken {
            envr_core::i18n::tr_key(
                "cli.doctor.fix.broken_current_ok",
                "{kind}：current 已从不存在的版本重定向到 {version}",
                "{kind}: repointed current from a missing version to {version}",
            )
        } else {
            envr_core::i18n::tr_key(
                "cli.doctor.fix.current_ok",
                "{kind}：已将 current 设为 {version}",
                "{kind}: set current to {version}",
            )
        };
        applied.push(fmt_template(
            &tmpl,
            &[("kind", kind_label(kind)), ("version", &best.0)],
        ));
    }

    applied
}
