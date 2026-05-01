#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestKind {
    Exact,
    Prefix,
    Alias,
    Range,
    Channel,
    Unknown,
}

pub(crate) fn classify_request(spec: Option<&str>, has_pin: bool) -> (RequestKind, Option<String>) {
    let raw = spec.map(str::trim).filter(|s| !s.is_empty());
    let Some(raw) = raw else {
        return (RequestKind::Unknown, None);
    };
    let kind = if matches!(raw, "latest" | "stable" | "lts" | "system") {
        RequestKind::Alias
    } else if raw.contains('<') || raw.contains('>') || raw.starts_with("~>") {
        RequestKind::Range
    } else if raw.contains('-')
        && raw.chars().any(|c| c.is_ascii_digit())
        && raw.chars().any(|c| c.is_ascii_alphabetic())
    {
        RequestKind::Channel
    } else if raw.chars().next().is_some_and(|c| c.is_ascii_digit()) && raw.split('.').count() < 3 {
        RequestKind::Prefix
    } else if raw.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        RequestKind::Exact
    } else if has_pin {
        RequestKind::Exact
    } else {
        RequestKind::Unknown
    };
    (kind, Some(raw.to_string()))
}

pub(crate) fn request_kind_str(kind: RequestKind) -> &'static str {
    match kind {
        RequestKind::Exact => "exact",
        RequestKind::Prefix => "prefix",
        RequestKind::Alias => "alias",
        RequestKind::Range => "range",
        RequestKind::Channel => "channel",
        RequestKind::Unknown => "unknown",
    }
}
