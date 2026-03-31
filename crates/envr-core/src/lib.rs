use envr_error::{EnvrError, EnvrResult};
use std::path::Path;

pub fn read_text_file(path: impl AsRef<Path>) -> EnvrResult<String> {
    std::fs::read_to_string(path).map_err(EnvrError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_returns_unified_error_for_missing_file() {
        let result = read_text_file("this-file-should-not-exist.envr");
        let err = result.expect_err("expected missing file to fail");
        assert_eq!(err.code(), envr_error::ErrorCode::Io);
    }
}
