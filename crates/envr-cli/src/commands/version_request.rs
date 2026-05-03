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
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VersionRequest {
    Exact(String),
    Prefix { major: u64, minor: Option<u64> },
    Alias(String),
    Range(String),
    Channel(String),
    System,
    Unknown,
}

impl ClassifiedRequest {
    pub(crate) fn kind_str(&self) -> &'static str {
        request_kind_str(self.kind)
    }

    pub(crate) fn request(&self) -> VersionRequest {
        classified_to_version_request(self)
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

fn is_numeric_prefix(spec: &str) -> bool {
    spec.chars().next().is_some_and(|c| c.is_ascii_digit())
        && prefix_depth(spec) < 3
        && spec.chars().all(|c| c.is_ascii_digit() || c == '.')
}

pub(crate) fn classify_request(spec: Option<&str>, has_pin: bool) -> ClassifiedRequest {
    let raw = spec
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let normalized = raw.as_deref().and_then(|s| normalize_request_spec(Some(s)));
    let Some(normalized_ref) = normalized.as_deref() else {
        return ClassifiedRequest {
            kind: RequestKind::Unknown,
            raw,
            normalized,
            alias: None,
        };
    };
    let alias = match normalized_ref.to_ascii_lowercase().as_str() {
        "latest" | "stable" | "lts" => Some(normalized_ref.to_string()),
        _ => None,
    };
    let kind = if normalized_ref == "system" {
        RequestKind::System
    } else if alias.is_some() {
        RequestKind::Alias
    } else if is_range_request(normalized_ref) {
        RequestKind::Range
    } else if is_channel_request(normalized_ref) {
        RequestKind::Channel
    } else if is_numeric_prefix(normalized_ref) {
        RequestKind::Prefix
    } else if normalized_ref
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit())
    {
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
        alias,
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

pub(crate) fn explain_request(request: &ClassifiedRequest) -> &'static str {
    match request.kind {
        RequestKind::Exact => "exact version requested",
        RequestKind::Prefix => "prefix request resolved to the newest matching installed version",
        RequestKind::Alias => match request.alias.as_deref() {
            Some("latest") => "latest alias resolved by runtime policy",
            Some("stable") => "stable alias resolved by runtime policy",
            Some("lts") => "lts alias resolved by runtime policy",
            _ => "alias request resolved by runtime policy",
        },
        RequestKind::Range => "version range resolved by runtime policy",
        RequestKind::Channel => "channel request resolved by runtime policy",
        RequestKind::System => "system runtime requested",
        RequestKind::Unknown => "unclassified request",
    }
}

pub(crate) fn classified_to_version_request(request: &ClassifiedRequest) -> VersionRequest {
    match request.kind {
        RequestKind::Exact => request
            .normalized
            .clone()
            .map(VersionRequest::Exact)
            .unwrap_or(VersionRequest::Unknown),
        RequestKind::Prefix => request
            .normalized
            .as_deref()
            .and_then(parse_prefix_request)
            .map_or(VersionRequest::Unknown, |(major, minor)| {
                VersionRequest::Prefix { major, minor }
            }),
        RequestKind::Alias => request
            .alias
            .clone()
            .map(VersionRequest::Alias)
            .unwrap_or(VersionRequest::Unknown),
        RequestKind::Range => request
            .normalized
            .clone()
            .map(VersionRequest::Range)
            .unwrap_or(VersionRequest::Unknown),
        RequestKind::Channel => request
            .normalized
            .clone()
            .map(VersionRequest::Channel)
            .unwrap_or(VersionRequest::Unknown),
        RequestKind::System => VersionRequest::System,
        RequestKind::Unknown => VersionRequest::Unknown,
    }
}

fn parse_prefix_request(spec: &str) -> Option<(u64, Option<u64>)> {
    let mut parts = spec.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = match parts.next() {
        Some(part) if !part.is_empty() => Some(part.parse().ok()?),
        _ => None,
    };
    Some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::{
        RequestKind, classify_request, explain_request, normalize_request_spec, request_kind_str,
    };

    #[test]
    fn normalize_request_strips_whitespace_and_single_v_prefix() {
        assert_eq!(
            normalize_request_spec(Some("  v22.11.0  ")),
            Some("22.11.0".to_string())
        );
        assert_eq!(
            normalize_request_spec(Some("22.11.0")),
            Some("22.11.0".to_string())
        );
        assert_eq!(
            normalize_request_spec(Some("vv22")),
            Some("v22".to_string())
        );
        assert_eq!(normalize_request_spec(Some("   ")), None);
        assert_eq!(normalize_request_spec(None), None);
    }

    #[test]
    fn classifies_aliases_and_prefixes() {
        assert_eq!(
            classify_request(Some("latest"), false).kind,
            RequestKind::Alias
        );
        assert_eq!(
            classify_request(Some("Latest"), false).kind,
            RequestKind::Alias
        );
        assert_eq!(
            classify_request(Some("vlatest"), false).kind,
            RequestKind::Alias
        );
        assert_eq!(
            classify_request(Some("stable"), false).kind,
            RequestKind::Alias
        );
        assert_eq!(
            classify_request(Some("lts"), false).kind,
            RequestKind::Alias
        );
        assert_eq!(
            classify_request(Some("system"), false).kind,
            RequestKind::System
        );
        assert_eq!(
            classify_request(Some("v22"), false).kind,
            RequestKind::Prefix
        );
        assert_eq!(
            classify_request(Some("22"), false).kind,
            RequestKind::Prefix
        );
        assert_eq!(
            classify_request(Some("22.11"), false).kind,
            RequestKind::Prefix
        );
        assert_eq!(
            classify_request(Some("22.11.0"), false).kind,
            RequestKind::Exact
        );
        assert_eq!(
            classify_request(Some("v22.11.0"), false).kind,
            RequestKind::Exact
        );
        assert_eq!(
            classify_request(Some("22.11.0.1"), false).kind,
            RequestKind::Exact
        );
    }

    #[test]
    fn classifies_ranges_and_channels() {
        assert_eq!(
            classify_request(Some(">=1.20 <1.23"), false).kind,
            RequestKind::Range
        );
        assert_eq!(
            classify_request(Some("~> 1.9"), false).kind,
            RequestKind::Range
        );
        assert_eq!(
            classify_request(Some("temurin-21"), false).kind,
            RequestKind::Channel
        );
        assert_eq!(
            classify_request(Some("graalvm-21.0.2"), false).kind,
            RequestKind::Channel
        );
    }

    #[test]
    fn classifies_aliases_case_insensitively() {
        assert_eq!(
            classify_request(Some("LATEST"), false).kind,
            RequestKind::Alias
        );
        assert_eq!(
            classify_request(Some("StAbLe"), false).kind,
            RequestKind::Alias
        );
        assert_eq!(
            classify_request(Some("LTS"), false).kind,
            RequestKind::Alias
        );
    }

    #[test]
    fn classifies_unknown_when_empty_and_no_pin() {
        assert_eq!(classify_request(None, false).kind, RequestKind::Unknown);
        assert_eq!(
            classify_request(Some("   "), false).kind,
            RequestKind::Unknown
        );
    }

    #[test]
    fn classifies_pinned_unknown_as_exact() {
        assert_eq!(
            classify_request(Some("latest"), true).kind,
            RequestKind::Alias
        );
        assert_eq!(
            classify_request(Some("custom"), true).kind,
            RequestKind::Exact
        );
        assert_eq!(
            classify_request(Some("   "), true).kind,
            RequestKind::Unknown
        );
    }

    #[test]
    fn request_kind_str_matches_kind() {
        assert_eq!(request_kind_str(RequestKind::Exact), "exact");
        assert_eq!(request_kind_str(RequestKind::Unknown), "unknown");
    }

    #[test]
    fn explains_aliases_by_name() {
        let latest = classify_request(Some("latest"), false);
        assert_eq!(
            explain_request(&latest),
            "latest alias resolved by runtime policy"
        );
        let stable = classify_request(Some("stable"), false);
        assert_eq!(
            explain_request(&stable),
            "stable alias resolved by runtime policy"
        );
        let lts = classify_request(Some("lts"), false);
        assert_eq!(
            explain_request(&lts),
            "lts alias resolved by runtime policy"
        );
    }

    #[test]
    fn normalizes_and_classifies_version_requests() {
        let r = classify_request(Some("v22.11.0"), false);
        assert_eq!(r.kind, RequestKind::Exact);
        assert_eq!(r.normalized.as_deref(), Some("22.11.0"));
    }

    #[test]
    fn explains_range_channel_and_system_requests() {
        let range = classify_request(Some(">=1.20 <1.23"), false);
        assert_eq!(
            explain_request(&range),
            "version range resolved by runtime policy"
        );
        let channel = classify_request(Some("temurin-21"), false);
        assert_eq!(
            explain_request(&channel),
            "channel request resolved by runtime policy"
        );
        let system = classify_request(Some("system"), false);
        assert_eq!(explain_request(&system), "system runtime requested");
    }
}
