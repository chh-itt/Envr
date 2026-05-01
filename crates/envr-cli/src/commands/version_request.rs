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
    let normalized = raw.strip_prefix('v').unwrap_or(raw);
    let kind = if matches!(normalized, "latest" | "stable" | "lts" | "system") {
        RequestKind::Alias
    } else if normalized.contains('<') || normalized.contains('>') || normalized.starts_with("~>") {
        RequestKind::Range
    } else if normalized.contains('-')
        && normalized.chars().any(|c| c.is_ascii_digit())
        && normalized.chars().any(|c| c.is_ascii_alphabetic())
    {
        RequestKind::Channel
    } else if normalized.chars().next().is_some_and(|c| c.is_ascii_digit()) && normalized.split('.').count() < 3 {
        RequestKind::Prefix
    } else if normalized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
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

#[cfg(test)]
mod tests {
    use super::{classify_request, request_kind_str, RequestKind};

    #[test]
    fn classifies_aliases_and_prefixes() {
        assert_eq!(classify_request(Some("latest"), false).0, RequestKind::Alias);
        assert_eq!(classify_request(Some("stable"), false).0, RequestKind::Alias);
        assert_eq!(classify_request(Some("lts"), false).0, RequestKind::Alias);
        assert_eq!(classify_request(Some("v22"), false).0, RequestKind::Prefix);
        assert_eq!(classify_request(Some("22"), false).0, RequestKind::Prefix);
        assert_eq!(classify_request(Some("22.11"), false).0, RequestKind::Prefix);
        assert_eq!(classify_request(Some("22.11.0"), false).0, RequestKind::Exact);
        assert_eq!(classify_request(Some("v22.11.0"), false).0, RequestKind::Exact);
    }

    #[test]
    fn classifies_ranges_and_channels() {
        assert_eq!(classify_request(Some(">=1.20 <1.23"), false).0, RequestKind::Range);
        assert_eq!(classify_request(Some("~> 1.9"), false).0, RequestKind::Range);
        assert_eq!(classify_request(Some("temurin-21"), false).0, RequestKind::Channel);
        assert_eq!(classify_request(Some("graalvm-21.0.2"), false).0, RequestKind::Channel);
    }

    #[test]
    fn classifies_unknown_when_empty_and_no_pin() {
        assert_eq!(classify_request(None, false).0, RequestKind::Unknown);
        assert_eq!(classify_request(Some("   "), false).0, RequestKind::Unknown);
    }

    #[test]
    fn request_kind_str_matches_kind() {
        assert_eq!(request_kind_str(RequestKind::Exact), "exact");
        assert_eq!(request_kind_str(RequestKind::Unknown), "unknown");
    }
}
