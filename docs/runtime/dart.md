# Dart runtime

`envr` supports standalone Dart SDK as first-class runtime `dart`.

## Remote source

- Source: Google `dart-archive` stable channel.
- Version discovery: GCS bucket prefix listing API.
- Artifact URL scheme:
  `https://storage.googleapis.com/dart-archive/channels/stable/release/<version>/sdk/dartsdk-<platform>-<arch>-release.zip`

## Install layout

- Version root: `runtimes/dart/versions/<version>/`
- Current pointer: `runtimes/dart/current`
- Home env for shims: `DART_HOME`
- Runtime env marker: `ENVR_DART_VERSION`

## Commands

```bash
envr remote dart
envr remote dart -u
envr install dart 3.11
envr use dart 3.11
envr which dart
envr exec --lang dart -- dart --version
```

## PATH proxy

- GUI: Env Center -> Dart -> PATH proxy
- Config:

```toml
[runtime.dart]
path_proxy_enabled = true
```

When disabled, `dart` shim passthrough goes to next system PATH candidate.
