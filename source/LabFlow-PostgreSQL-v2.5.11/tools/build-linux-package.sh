#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${1:-$ROOT_DIR/../releases}"
VERSION="$(tr -d '\r\n' < "$ROOT_DIR/VERSION")"
BUILD_DIR="$(mktemp -d "${TMPDIR:-/tmp}/labflow-v${VERSION}-XXXXXX")"
trap 'rm -rf -- "$BUILD_DIR"' EXIT
STAGE_DIR="$BUILD_DIR/stage"
DEB_DIR="$STAGE_DIR/DEBIAN"

command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }
command -v npm >/dev/null || { echo "npm is required" >&2; exit 1; }
command -v dpkg-deb >/dev/null || { echo "dpkg-deb is required" >&2; exit 1; }

rm -rf "$BUILD_DIR"
mkdir -p "$DEB_DIR" "$STAGE_DIR/opt/labflow/bin"

pushd "$ROOT_DIR/frontend" >/dev/null
npm ci --ignore-scripts
npm run build
popd >/dev/null

pushd "$ROOT_DIR" >/dev/null
cargo build --release --locked
popd >/dev/null

cp "$ROOT_DIR/target/release/labflow" "$STAGE_DIR/opt/labflow/bin/labflow"
cp -R "$ROOT_DIR/backend/static" "$STAGE_DIR/opt/labflow/bin/static"
cp "$ROOT_DIR/config.example.toml" "$STAGE_DIR/opt/labflow/config.example.toml"

install -m 0644 "$ROOT_DIR/packaging/debian/control" "$DEB_DIR/control"
install -m 0755 "$ROOT_DIR/packaging/debian/postinst" "$DEB_DIR/postinst"
install -m 0755 "$ROOT_DIR/packaging/debian/prerm" "$DEB_DIR/prerm"
install -m 0755 "$ROOT_DIR/packaging/debian/postrm" "$DEB_DIR/postrm"
mkdir -p "$STAGE_DIR/lib/systemd/system"
cp "$ROOT_DIR/packaging/debian/labflow.service" "$STAGE_DIR/lib/systemd/system/labflow.service"

mkdir -p "$OUT_DIR"
dpkg-deb --build --root-owner-group "$STAGE_DIR" "$OUT_DIR/labflow_${VERSION}_amd64.deb"
echo "Built $OUT_DIR/labflow_${VERSION}_amd64.deb"
