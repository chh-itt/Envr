#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestKind {
    Exact,
    Prefix,
    Alias,
    Range,
    Channel,
    System,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassifiedRequest {
    pub kind: RequestKind,
    pub raw: Option<String>,
    pub normalized: Option<String>,
}

impl ClassifiedRequest {
    pub(crate) fn kind_str(&self) -> &'static str {
        request_kind_str(self.kind)
    }
}

pub(crate) fn normalize_request_spec(spec: Option<&str>) -> Option<String> {
    let raw = spec.map(str::trim).filter(|s| !s.is_empty())?;
    let normalized = raw.strip_prefix('v').unwrap_or(raw).to_string();
    Some(normalized)
}

fn is_range_request(spec: &str) -> bool {
    spec.contains('<') || spec.contains('>') || spec.starts_with("~>")
}

fn is_channel_request(spec: &str) -> bool {
    spec.contains('-')
        && spec.chars().any(|c| c.is_ascii_digit())
        && spec.chars().any(|c| c.is_ascii_alphabetic())
}

fn prefix_depth(spec: &str) -> usize {
    spec.split('.').take_while(|part| !part.is_empty()).count()
}

pub(crate) fn classify_request(spec: Option<&str>, has_pin: bool) -> ClassifiedRequest {
    let raw = spec.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let normalized = raw.as_deref().and_then(|s| normalize_request_spec(Some(s)));
    let Some(normalized_ref) = normalized.as_deref() else {
        return ClassifiedRequest {
            kind: RequestKind::Unknown,
            raw,
            normalized,
        };
    };
    let kind = if normalized_ref == "system" {
        RequestKind::System
    } else if matches!(normalized_ref, "latest" | "stable" | "lts") {
        RequestKind::Alias
    } else if is_range_request(normalized_ref) {
        RequestKind::Range
    } else if is_channel_request(normalized_ref) {
        RequestKind::Channel
    } else if normalized_ref.chars().next().is_some_and(|c| c.is_ascii_digit())
        && prefix_depth(normalized_ref) < 3
        && normalized_ref.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        RequestKind::Prefix
    } else if normalized_ref.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        RequestKind::Exact
    } else if has_pin {
        RequestKind::Exact
    } else {
        RequestKind::Unknown
    };
    ClassifiedRequest {
        kind,
        raw,
        normalized,
    }
}

pub(crate) fn request_kind_str(kind: RequestKind) -> &'static str {
    match kind {
        RequestKind::Exact => "exact",
        RequestKind::Prefix => "prefix",
        RequestKind::Alias => "alias",
        RequestKind::Range => "range",
        RequestKind::Channel => "channel",
        RequestKind::System => "system",
        RequestKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_request, normalize_request_spec, request_kind_str, RequestKind};

    #[test]
    fn normalize_request_strips_whitespace_and_single_v_prefix() {
        assert_eq!(normalize_request_spec(Some("  v22.11.0  ")), Some("22.11.0".to_string()));
        assert_eq!(normalize_request_spec(Some("22.11.0")), Some("22.11.0".to_string()));
        assert_eq!(normalize_request_spec(Some("vv22")), Some("v22".to_string()));
        assert_eq!(normalize_request_spec(Some("   ")), None);
        assert_eq!(normalize_request_spec(None), None);
    }

    #[test]
    fn classifies_aliases_and_prefixes() {
        assert_eq!(classify_request(Some("latest"), false).kind, RequestKind::Alias);
        assert_eq!(classify_request(Some("stable"), false).kind, RequestKind::Alias);
        assert_eq!(classify_request(Some("lts"), false).kind, RequestKind::Alias);
        assert_eq!(classify_request(Some("system"), false).kind, RequestKind::System);
        assert_eq!(classify_request(Some("v22"), false).kind, RequestKind::Prefix);
        assert_eq!(classify_request(Some("22"), false).kind, RequestKind::Prefix);
        assert_eq!(classify_request(Some("22.11"), false).kind, RequestKind::Prefix);
        assert_eq!(classify_request(Some("22.11.0"), false).kind, RequestKind::Exact);
        assert_eq!(classify_request(Some("v22.11.0"), false).kind, RequestKind::Exact);
    }

    #[test]
    fn classifies_ranges_and_channels() {
        assert_eq!(classify_request(Some(">=1.20 <1.23"), false).kind, RequestKind::Range);
        assert_eq!(classify_request(Some("~> 1.9"), false).kind, RequestKind::Range);
        assert_eq!(classify_request(Some("temurin-21"), false).kind, RequestKind::Channel);
        assert_eq!(classify_request(Some("graalvm-21.0.2"), false).kind, RequestKind::Channel);
    }

    #[test]
    fn classifies_unknown_when_empty_and_no_pin() {
        assert_eq!(classify_request(None, false).kind, RequestKind::Unknown);
        assert_eq!(classify_request(Some("   "), false).kind, RequestKind::Unknown);
    }

    #[test]
    fn classifies_pinned_unknown_as_exact() {
        assert_eq!(classify_request(Some("latest"), true).kind, RequestKind::Alias);
        assert_eq!(classify_request(Some("custom"), true).kind, RequestKind::Exact);
        assert_eq!(classify_request(Some("   "), true).kind, RequestKind::Unknown);
    }

    #[test]
    fn request_kind_str_matches_kind() {
        assert_eq!(request_kind_str(RequestKind::Exact), "exact");
        assert_eq!(request_kind_str(RequestKind::Unknown), "unknown");
    }
}
