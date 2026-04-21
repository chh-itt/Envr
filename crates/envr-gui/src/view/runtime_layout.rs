//! Resolve runtime hub / dashboard order and visibility from `settings.toml` `[gui.runtime_layout]`.

use std::collections::{HashMap, HashSet};

use envr_config::settings::RuntimeLayoutSettings;
use envr_domain::runtime::{RuntimeKind, runtime_descriptor, runtime_kinds_all};

/// Messages for reordering / hiding runtimes (dashboard + settings; persisted under `gui.runtime_layout`).
#[derive(Debug, Clone)]
pub enum RuntimeLayoutMsg {
    ToggleDashboardLayoutEditing,
    ToggleDashboardHiddenCollapsed,
    ToggleHidden(RuntimeKind),
    MoveRuntime { kind: RuntimeKind, delta: isize },
    OpenRuntime(RuntimeKind),
    ResetToDefaults,
}

use crate::view::dashboard::RuntimeRow;

fn kind_from_key(key: &str) -> Option<RuntimeKind> {
    runtime_kinds_all().find(|k| runtime_descriptor(*k).key == key)
}

fn default_key_order() -> Vec<String> {
    runtime_kinds_all()
        .map(|k| runtime_descriptor(k).key.to_string())
        .collect()
}

/// Effective key order (full permutation, known keys only, missing keys appended in default order).
pub fn effective_key_order(layout: &RuntimeLayoutSettings) -> Vec<String> {
    let default_keys = default_key_order();
    let known: HashSet<&str> = default_keys.iter().map(|s| s.as_str()).collect();
    if layout.order.is_empty() {
        return default_keys;
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for k in &layout.order {
        if known.contains(k.as_str()) && seen.insert(k.as_str()) {
            out.push(k.clone());
        }
    }
    for dk in &default_keys {
        if !seen.contains(dk.as_str()) {
            out.push(dk.clone());
        }
    }
    out
}

pub fn hidden_key_set(layout: &RuntimeLayoutSettings) -> HashSet<String> {
    let default_keys = default_key_order();
    let known: HashSet<&str> = default_keys.iter().map(|s| s.as_str()).collect();
    layout
        .hidden
        .iter()
        .filter(|k| known.contains(k.as_str()))
        .cloned()
        .collect()
}

pub fn effective_kinds_order(layout: &RuntimeLayoutSettings) -> Vec<RuntimeKind> {
    effective_key_order(layout)
        .iter()
        .filter_map(|k| kind_from_key(k))
        .collect()
}

pub fn visible_kinds(layout: &RuntimeLayoutSettings) -> Vec<RuntimeKind> {
    let hidden = hidden_key_set(layout);
    let mut v: Vec<RuntimeKind> = effective_kinds_order(layout)
        .into_iter()
        .filter(|k| !hidden.contains(runtime_descriptor(*k).key))
        .collect();
    if v.is_empty() {
        // Corrupt or legacy file: never leave the hub with zero tabs.
        v = effective_kinds_order(layout);
    }
    v
}

pub fn is_kind_hidden(layout: &RuntimeLayoutSettings, kind: RuntimeKind) -> bool {
    hidden_key_set(layout).contains(runtime_descriptor(kind).key)
}

/// Ensure `layout.order` is a full materialized permutation (for in-place swaps).
pub fn materialize_order(layout: &mut RuntimeLayoutSettings) {
    layout.order = effective_key_order(layout);
}

pub fn partition_dashboard_rows(
    layout: &RuntimeLayoutSettings,
    rows: &[RuntimeRow],
) -> (Vec<RuntimeRow>, Vec<RuntimeRow>) {
    let hidden = hidden_key_set(layout);
    let map: HashMap<RuntimeKind, RuntimeRow> = rows.iter().cloned().map(|r| (r.kind, r)).collect();
    let mut visible = Vec::new();
    let mut hidden_rows = Vec::new();
    for k in effective_kinds_order(layout) {
        if let Some(r) = map.get(&k) {
            if hidden.contains(runtime_descriptor(k).key) {
                hidden_rows.push(r.clone());
            } else {
                visible.push(r.clone());
            }
        }
    }
    (visible, hidden_rows)
}

pub fn toggle_hidden_key(layout: &mut RuntimeLayoutSettings, key: &str) {
    materialize_order(layout);
    let mut hs = hidden_key_set(layout);
    if hs.remove(key) {
        layout.hidden = effective_key_order(layout)
            .into_iter()
            .filter(|k| hs.contains(k.as_str()))
            .collect();
        return;
    }
    let visible_now = effective_key_order(layout)
        .iter()
        .filter(|k| !hs.contains(k.as_str()))
        .count();
    if visible_now <= 1 {
        return;
    }
    hs.insert(key.to_string());
    layout.hidden = effective_key_order(layout)
        .into_iter()
        .filter(|k| hs.contains(k.as_str()))
        .collect();
}

pub fn move_kind_delta(layout: &mut RuntimeLayoutSettings, kind: RuntimeKind, delta: isize) {
    materialize_order(layout);
    let key = runtime_descriptor(kind).key;
    let Some(idx) = layout.order.iter().position(|k| k == key) else {
        return;
    };
    let j = (idx as isize + delta).clamp(0, layout.order.len() as isize - 1) as usize;
    if idx != j {
        layout.order.swap(idx, j);
    }
}

pub fn reset_runtime_layout(layout: &mut RuntimeLayoutSettings) {
    layout.order.clear();
    layout.hidden.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_order_fills_missing() {
        let mut layout = RuntimeLayoutSettings::default();
        layout.order = vec!["python".into(), "node".into()];
        let keys = effective_key_order(&layout);
        assert!(
            keys.iter().position(|k| k == "python").unwrap()
                < keys.iter().position(|k| k == "node").unwrap()
        );
        assert_eq!(keys.len(), 27);
    }

    #[test]
    fn visible_filters_hidden() {
        let mut layout = RuntimeLayoutSettings::default();
        layout.hidden = vec!["bun".into()];
        assert!(!visible_kinds(&layout).contains(&RuntimeKind::Bun));
    }
}
