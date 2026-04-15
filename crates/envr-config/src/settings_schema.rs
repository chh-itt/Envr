//! Commented `settings.toml` template (Chinese) for `envr config schema`.

/// Full template: every major section, defaults, and brief 中文说明.
pub fn settings_toml_schema_template_zh() -> &'static str {
    include_str!("../templates/settings.schema.zh.toml")
}

#[cfg(test)]
mod tests {
    use super::settings_toml_schema_template_zh;
    use crate::settings::Settings;

    #[test]
    fn schema_template_roundtrips_parse() {
        let raw = settings_toml_schema_template_zh();
        let s: Settings = toml::from_str(raw).expect("commented schema template must deserialize");
        s.validate()
            .expect("schema template must satisfy Settings::validate");
    }
}
