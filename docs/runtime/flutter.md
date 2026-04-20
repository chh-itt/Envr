# Flutter runtime

`envr` supports standalone Flutter SDK as first-class runtime `flutter`.

## Remote source

- Source: Flutter official release metadata JSON (`flutter_infra_release`), stable channel.
- Host-specific feed:
  - Windows: `releases_windows.json`
  - Linux: `releases_linux.json`
  - macOS: `releases_macos.json`

## Install layout

- Version root: `runtimes/flutter/versions/<version>/`
- Current pointer: `runtimes/flutter/current`
- Home env for shims: `FLUTTER_HOME`
- Runtime env marker: `ENVR_FLUTTER_VERSION`

## Dart coexistence policy

- `dart` and `flutter` are independent runtimes.
- `flutter` installation does **not** remap `dart` shim automatically.

## Managed install policy

- By default, envr **keeps** `$FLUTTER_ROOT/.git` because Flutter tooling depends on repository metadata.
- Optional override: set `ENVR_FLUTTER_STRIP_GIT=1` to strip `.git` after install (not recommended for normal use).
- Flutter CLI also expects Git utilities on PATH (Windows includes `where.exe` from `%SystemRoot%\\System32`); if missing, `flutter --version` and other commands can fail.

## First-run behavior

- First invocation may run Flutter tool bootstrap steps (`Building flutter tool...`, `Running pub upgrade...`), which can take noticeably longer.
- After warmup, subsequent `flutter --version` calls are typically faster.

## Commands

```bash
envr remote flutter
envr remote flutter -u
envr install flutter 3.41
envr use flutter 3.41
envr which flutter
envr exec --lang flutter -- flutter --version
```

## PATH proxy

- GUI: Env Center -> Flutter -> PATH proxy
- Config:

```toml
[runtime.flutter]
path_proxy_enabled = true
```
