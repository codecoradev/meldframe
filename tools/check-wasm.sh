#!/usr/bin/env bash
# Builds the wasm bridge (debug, Node glue) into a temporary directory and runs tools/wasm-smoke.mjs on it.
set -euo pipefail
cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
PROFILE=debug WASM_TARGET=nodejs bash tools/build-wasm.sh "$tmp" >/dev/null
node tools/wasm-smoke.mjs "$tmp"
