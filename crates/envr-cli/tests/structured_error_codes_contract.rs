use envr_cli::output::{error_code_token, exit_code_for_error_code};
use envr_error::ErrorCode;

#[test]
fn structured_runtime_error_tokens_are_stable() {
    assert_eq!(
        error_code_token(ErrorCode::RuntimeVersionSpecInvalid),
        "runtime_version_spec_invalid"
    );
    assert_eq!(
        error_code_token(ErrorCode::RuntimeVersionNotFound),
        "runtime_version_not_found"
    );
    assert_eq!(
        error_code_token(ErrorCode::RemoteIndexFetchFailed),
        "remote_index_fetch_failed"
    );
    assert_eq!(
        error_code_token(ErrorCode::RemoteIndexParseFailed),
        "remote_index_parse_failed"
    );
}

#[test]
fn structured_runtime_error_exit_codes_are_stable() {
    assert_eq!(
        exit_code_for_error_code(ErrorCode::RuntimeVersionSpecInvalid),
        1
    );
    assert_eq!(
        exit_code_for_error_code(ErrorCode::RuntimeVersionNotFound),
        1
    );
    assert_eq!(
        exit_code_for_error_code(ErrorCode::RemoteIndexParseFailed),
        1
    );
    assert_eq!(
        exit_code_for_error_code(ErrorCode::RemoteIndexFetchFailed),
        2
    );
}
