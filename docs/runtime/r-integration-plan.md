# R language runtime integration (envr)

## 1) Why R (and naming)

- **Language R** (CRAN) is distinct from **`RuntimeKind::Rust`** (key `rust`). This runtime uses **`RuntimeKind::RLang`** and descriptor key **`r`** so `.envr.toml` is `[runtimes.r]` and CLI is `envr install r 4.4.2`.
- **Official Windows builds** are Inno Setup installers (`R-x.y.z-win.exe`), not a portable zip. envr runs **`/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR=...`** into `runtimes/r/versions/<ver>/`.
- **Version index**: [rversions.r-pkg.org](https://rversions.r-pkg.org/) exposes JSON (`r-versions`, `r-release-win`) so we do not scrape CRAN HTML for the list. **CRAN URLs** for each installer follow the well-known `base/` vs `base/old/<ver>/` split relative to the current **latest** Windows release.

## 2) Goal and scope (MVP)

- `RuntimeKind::RLang`, crate **`envr-runtime-rlang`**, registered in `RuntimeService`.
- **Windows x86_64 (and aarch64-pc-windows-msvc where CRAN ships the same `*-win.exe` pattern)** only for **managed install**; other hosts return a clear validation error from `list_remote` / `install` (no fake empty list).
- Shims: **`R`** and **`Rscript`** (PATH proxy like other runtimes).
- `R_HOME` set for `exec` / `run` / shims (matches common R tooling expectations).
- `.envr.toml`: `[runtimes.r] version = "4.4.2"`.
- Two-part **major line** keys (`4.4`, `4.3`) like Julia/Nim via `version_line_key_for_kind`.

### Out of scope (MVP)

- **Linux / macOS** managed install (different artifact types: `.deb`, `.pkg`, or rig); document follow-up.
- **Rterm**, **R CMD** as separate shims (only `R` / `Rscript` core surface).
- **PPM / rig** as installer backends.

## 3) Layout

- `runtimes/r/versions/<version>/` — full R prefix after silent install (`bin/R.exe`, `bin/Rscript.exe`, library tree).
- `runtimes/r/current` — symlink or Windows pointer file (Julia/Nim pattern).

## 4) Acceptance / tests

- `cargo test -p envr-runtime-rlang` with JSON fixture for `r-versions` slice + URL helper unit tests.
- Manual (Windows): `envr remote r -u`, `envr install r 4.4.2`, `envr use r 4.4.2`, `envr exec --lang r -- Rscript --version`, `envr shim sync`.

## 5) Architecture / friction log

- **Inno installer vs archive runtimes**: cannot reuse generic “download zip + extract only”; install path is **spawn installer + verify `bin/R.exe`**. Document in playbook under “installer-backed runtimes”.
- **rversions dependency**: third-party JSON service (same class of risk as any index); mitigate with disk cache + TTL env vars.
- **Non-Windows UX**: remote/install error strings must be explicit so GUI remote banner is actionable.

## 6) Development log

- [x] Plan (`docs/runtime/r-integration-plan.md`).
- [x] Crate `envr-runtime-rlang` (index, manager, provider, fixtures).
- [x] Domain / config (`runtime_path_proxy`, `RuntimeSettings` field `r` → TOML `[runtime.r]`) / core / shim / resolver / CLI / GUI / docs / playbook touch-up.

### CLI / GUI follow-ups (post-merge)

- Verify silent install under a **user-writable** `ENVR_RUNTIME_ROOT` (no elevation).
- If CRAN changes `old/` URL rules, adjust `cran_windows_r_installer_url` and extend fixture tests.
- **PATH dry-run assertion:** `exec --dry-run` prints `PATH=...`; on Windows the segment may spell `...\r\...` (short key `r`); broad substring checks can false-positive—prefer `R_HOME=` plus a version directory segment you control.
