# GitHub Release Runtime Template

This template is for `envr-runtime-*` crates that fetch installable versions from GitHub releases.

## Goal

- Keep per-runtime code focused on platform asset matching and local install layout.
- Reuse shared release-index logic from `envr-runtime-github-release`.

## Required dependencies

In runtime crate `Cargo.toml`:

```toml
envr-runtime-github-release = { path = "../envr-runtime-github-release" }
```

## Minimal `index.rs` structure

1. Keep runtime-specific types:
   - `<Runtime>InstallableRow { version, url }`
   - `DEFAULT_*_RELEASES_API_URL`
   - version tag parser and semver comparator
2. Import shared primitives:
   - `GhRepo`
   - `GhAsset`, `GhRelease` (directly or `pub use`)
3. Keep runtime-specific callbacks:
   - `label_from_tag(tag) -> Option<String>`
   - `pick_asset(...)` or `asset_filename(...)`
   - `make_synthetic_url(tag, version) -> Option<String>`
4. Call shared pipeline:
   - `fetch_github_releases_index(client, releases_api_url, DEFAULT_...)`
   - `fetch_rows_via_html(client, repo, label_from_tag, make_synthetic_url, cmp)`
   - `fetch_rows_via_atom(client, repo, label_from_tag, make_synthetic_url, cmp)`

## Fallback contract

- First try GitHub API pages.
- If API yields no usable rows, fall back to releases HTML tag scan.
- If still empty, fall back to releases Atom feed.
- Runtime keeps ownership of asset naming policy and version normalization.

## Example skeleton

```rust
const RUNTIME_REPO: GhRepo = GhRepo { owner: "org", name: "repo" };

pub fn fetch_runtime_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<RuntimeInstallableRow>> {
    if let Ok(releases) = envr_runtime_github_release::fetch_github_releases_index(
        client,
        releases_api_url,
        DEFAULT_RUNTIME_RELEASES_API_URL,
    ) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }

    if let Ok(rows) = envr_runtime_github_release::fetch_rows_via_html(
        client,
        RUNTIME_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    ) && !rows.is_empty()
    {
        return Ok(rows
            .into_iter()
            .map(|r| RuntimeInstallableRow { version: r.version, url: r.url })
            .collect());
    }

    let rows = envr_runtime_github_release::fetch_rows_via_atom(
        client,
        RUNTIME_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| RuntimeInstallableRow { version: r.version, url: r.url })
        .collect())
}
```

## Migration checklist

- Move token/header/pagination/proxy-unwrapping into shared crate calls.
- Keep runtime-specific tag parsing unchanged.
- Keep runtime-specific asset candidate order unchanged.
- Preserve existing sort/dedup semantics for row and version outputs.
- Run:
  - `cargo check -p envr-runtime-github-release`
  - `cargo check -p <runtime-crate>`
