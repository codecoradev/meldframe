#!/usr/bin/env bash
# Builds crates/app/meldframe-wasm for the browser into src/wasm (git-ignored): cargo for
# wasm32-unknown-unknown, then the wasm-bindgen CLI (its version must equal the wasm-bindgen crate in
# Cargo.lock; AGENTS.md says how to install it). Pass --release for an optimised build.
set -euo pipefail
cd "$(dirname "$0")/../../.."
profile=debug; flag=()
if [ "${1:-}" = "--release" ]; then profile=release; flag=(--release); fi
cargo build -p meldframe-wasm --target wasm32-unknown-unknown "${flag[@]}"
target="${CARGO_TARGET_DIR:-target}"
wasm-bindgen --target web --out-dir apps/web/src/wasm --out-name meldframe_wasm \
  "$target/wasm32-unknown-unknown/$profile/meldframe_wasm.wasm"
