# Terraform runtime support

`envr` supports Terraform as a managed runtime (`RuntimeKind::Terraform`), including:

- remote version discovery (`envr remote terraform`)
- managed install/use (`envr install terraform <spec>`, `envr use terraform <version>`)
- shim command (`terraform`)
- `exec` / `run` environment merge (`TERRAFORM_HOME`)

## Remote source and cache

- Source index: `https://releases.hashicorp.com/terraform/`
- Artifact pattern: `terraform_<version>_<platform>.zip`

Cache location:

- `{runtime_root}/cache/terraform/index_versions.json`

TTL environment variable:

- `ENVR_TERRAFORM_INDEX_CACHE_TTL_SECS` (default 21600 seconds / 6h)

## PATH proxy toggle

`settings.toml`:

```toml
[runtime.terraform]
path_proxy_enabled = true
```

When disabled, `terraform` shim passthrough goes to system PATH, and managed "Use / Install & Use" actions are blocked in GUI.

## Quick checks

```powershell
envr remote terraform
envr remote terraform -u
envr install terraform 1.14
envr use terraform 1.14.8
envr exec --lang terraform -- terraform version
envr which terraform
```
