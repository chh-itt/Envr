#!/usr/bin/env bash
# Build release binaries and produce a versioned tar.gz + SHA256 checksums (Linux/macOS).
# Usage (from repo root): ./scripts/package-unix-release.sh --version 0.1.0 --platform linux --arch x86_64 [--target x86_64-unknown-linux-gnu] [--out-root dist]
set -euo pipefail

VERSION="0.1.0"
PLATFORM=""
ARCH=""
TARGET=""
OUT_ROOT="dist"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      VERSION="$2"
      shift 2
      ;;
    --platform)
      PLATFORM="$2"
      shift 2
      ;;
    --arch)
      ARCH="$2"
      shift 2
      ;;
    --target)
      TARGET="$2"
      shift 2
      ;;
    --out-root)
      OUT_ROOT="$2"
      shift 2
      ;;
    -h|--help)
      sed -n '1,3p' "$0"
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z "${PLATFORM}" || -z "${ARCH}" ]]; then
  echo "--platform and --arch are required" >&2
  exit 2
fi

case "${PLATFORM}" in
  linux|macos) ;;
  *)
    echo "unsupported platform: ${PLATFORM}" >&2
    exit 2
    ;;
esac

case "${ARCH}" in
  x86_64|arm64) ;;
  *)
    echo "unsupported arch: ${ARCH}" >&2
    exit 2
    ;;
esac

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

PACKAGE_NAME="envr-${PLATFORM}-${ARCH}-${VERSION}"
DEST="${OUT_ROOT}/${PACKAGE_NAME}"

rm -rf "${DEST}"
mkdir -p "${DEST}"

echo "Building release (${PLATFORM} ${ARCH})..."
if [[ -n "${TARGET}" ]]; then
  if [[ "${TARGET}" == "aarch64-unknown-linux-gnu" ]]; then
    export CC_aarch64_unknown_linux_gnu="aarch64-linux-gnu-gcc"
    export CXX_aarch64_unknown_linux_gnu="aarch64-linux-gnu-g++"
    export AR_aarch64_unknown_linux_gnu="aarch64-linux-gnu-ar"
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="aarch64-linux-gnu-gcc"
  fi
  cargo build --release --target "${TARGET}" -p envr-cli -p envr-shim
  BIN_DIR="target/${TARGET}/release"
else
  cargo build --release -p envr-cli -p envr-shim
  BIN_DIR="target/release"
fi

bins=("envr" "er" "envr-shim")
for bin in "${bins[@]}"; do
  src="${BIN_DIR}/${bin}"
  if [[ ! -f "${src}" ]]; then
    echo "missing ${src} — build failed or binary name changed" >&2
    exit 1
  fi
  cp "${src}" "${DEST}/${bin}"
  chmod +x "${DEST}/${bin}"
done

(
  cd "${DEST}"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${bins[@]}" > SHA256SUMS.txt
  else
    shasum -a 256 "${bins[@]}" > SHA256SUMS.txt
  fi
)

archive="${OUT_ROOT}/${PACKAGE_NAME}.tar.gz"
rm -f "${archive}"
tar -C "${OUT_ROOT}" -czf "${archive}" "${PACKAGE_NAME}"

(
  cd "${OUT_ROOT}"
  checksum_file="SHA256SUMS-${PACKAGE_NAME}.txt"
  rm -f "${checksum_file}"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${PACKAGE_NAME}.tar.gz" > "${checksum_file}"
  else
    shasum -a 256 "${PACKAGE_NAME}.tar.gz" > "${checksum_file}"
  fi
)

echo "Done."
echo "  Folder: ${DEST}"
echo "  Archive: ${archive}"
