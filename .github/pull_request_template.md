## Summary

- Describe the main change in 1-3 bullet points.
- Focus on why this change is needed.

## Change type

- [ ] Bug fix
- [ ] Feature
- [ ] Refactor
- [ ] Docs
- [ ] Test
- [ ] Build / CI / release
- [ ] Other

## Affected areas

- [ ] CLI behavior
- [ ] JSON output / schemas / automation contract
- [ ] Runtime provider logic
- [ ] Download / mirror / cache / offline workflow
- [ ] Shim / PATH / shell integration
- [ ] Project config / `.envr.toml`
- [ ] GUI
- [ ] Docs only

## Test plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo test --workspace`
- [ ] Targeted manual verification completed
- [ ] Not applicable

## Manual verification

Describe any commands, platforms, or runtime scenarios you manually tested.

## Contract and compatibility notes

If this PR changes user-facing CLI behavior, JSON output, schemas, runtime resolution, or shell/shim behavior, describe the compatibility impact here.

## Documentation

- [ ] README updated if needed
- [ ] User-facing docs updated if behavior changed
- [ ] Contract/schema docs updated if needed
- [ ] No docs update needed

## Security considerations

Does this change affect any of the following?

- remote downloads or archive extraction
- checksum or integrity validation
- mirrors, offline caches, or bundle behavior
- shims, PATH handling, or shell integration
- diagnostics export or sensitive local paths

If yes, explain the risk and mitigation.
