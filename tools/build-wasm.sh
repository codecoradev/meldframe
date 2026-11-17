#!/usr/bin/env bash
# Builds meldframe-wasm for the browser: cargo for wasm32, then the wasm-bindgen glue.
# Usage: tools/build-wasm.sh [out-dir]   (default apps/web/.wasm, git-ignored)
# Environment: PROFILE=release|debug (default release), WASM_TARGET=web|nodejs (default web).
# The wasm-bindgen CLI must be the version of the wasm-bindgen crate in Cargo.lock:
#   cargo install wasm-bindgen-cli --version <that version> --locked
set -euo pipefail
cd "$(dirname "$0")/.."
# A relative out-dir is relative to the repository root (this script changes into it).
OUT="${1:-apps/web/.wasm}"
case "$OUT" in
  ""|"."|"./"|".."|"/"|"$HOME"|"$PWD"|"$PWD/") echo "build-wasm: refusing to replace '$OUT'" >&2; exit 1 ;;
esac
PROFILE="${PROFILE:-release}"
TARGET="${WASM_TARGET:-web}"

want=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock)
have=$(wasm-bindgen --version 2>/dev/null | awk '{ print $2 }' || true)
if [ -z "$have" ]; then
  echo "build-wasm: wasm-bindgen is not installed. Run: cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi
if [ "$have" != "$want" ]; then
  echo "build-wasm: wasm-bindgen $have does not match Cargo.lock ($want). Run: cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

flags=(--target wasm32-unknown-unknown -p meldframe-wasm)
[ "$PROFILE" = release ] && flags+=(--release)
cargo build "${flags[@]}"

wasm="${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/$PROFILE/meldframe_wasm.wasm"
rm -rf "$OUT" && mkdir -p "$OUT"
wasm-bindgen --target "$TARGET" --out-dir "$OUT" --out-name meldframe_wasm "$wasm"
echo "build-wasm: $OUT ($(du -h "$OUT/meldframe_wasm_bg.wasm" | cut -f1) wasm, $PROFILE, target $TARGET)"
