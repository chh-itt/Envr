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

## Dart/Flutter coexistence matrix

| Scenario | Expected behavior | Recommended command |
| --- | --- | --- |
| `envr use dart <v>` then run `dart --version` | Uses standalone Dart runtime current | `envr which dart` |
| `envr use flutter <v>` then run `flutter --version` | Uses Flutter runtime current | `envr which flutter` |
| `envr use flutter <v>` and then run `dart --version` | Still uses standalone Dart (not Flutter-embedded Dart) | `envr use dart <v>` to switch Dart explicitly |
| Need Flutter toolchain for one command only | Keep global current untouched, run in scoped env | `envr exec --lang flutter -- flutter doctor -v` |
| Need standalone Dart for one command only | Keep global current untouched, run in scoped env | `envr exec --lang dart -- dart pub get` |

### Common symptoms and fixes

- `flutter --version` reports Git/repository errors:
  - Ensure `.git` is present under the installed Flutter version directory.
  - Do not set `ENVR_FLUTTER_STRIP_GIT=1` for normal usage.
- `flutter --version` fails with missing `where` / missing Git on Windows:
  - Ensure `%SystemRoot%\\System32` is present in PATH (`where.exe` required).
  - Ensure Git is installed and available on PATH.

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
