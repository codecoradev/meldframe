#!/usr/bin/env bash
# Protocol pipeline check: regenerate the TypeScript types from Rust, compile a sample consumer against them and
# fail if the generated files differ from what is committed.
set -euo pipefail
cd "$(dirname "$0")/.."
GEN=packages/protocol/src/generated

rm -rf "$GEN" && mkdir -p "$GEN"
cargo test -p meldframe-protocol -p meldframe-app-model --quiet >/dev/null
# Deterministic barrel file.
(cd "$GEN" && for f in $(ls *.ts | grep -v '^index.ts$' | LC_ALL=C sort); do n=${f%.ts}; echo "export type { $n } from './$n';"; done) > "$GEN/index.ts"

# JSON has no bigint: an integer field of 64 bits needs `#[ts(type = "number")]`, or the types lie about the JSON.
if grep -rn 'bigint' "$GEN" >/dev/null; then
  echo "FAIL: a generated type uses bigint, but serde reads and writes a JSON number. Add #[ts(type = \"number\")] to the field:"
  grep -rn 'bigint' "$GEN" | cut -c1-160
  exit 1
fi

echo "[1/2] tsc (TypeScript consumer)"
if command -v tsc >/dev/null 2>&1; then
  tsc --noEmit -p packages/protocol
else
  npx --yes -p typescript@6.0.2 tsc --noEmit -p packages/protocol
fi

echo "[2/2] generated files match the committed ones"
# Include untracked files so a newly generated type that was never committed also fails.
if [ -n "$(git status --porcelain --untracked-files=all -- "$GEN")" ]; then
  echo "FAIL: generated TypeScript types differ from the committed ones. Run tools/check-protocol.sh and commit the result:"
  git status --short --untracked-files=all -- "$GEN"
  exit 1
fi
echo "OK: protocol pipeline is consistent"
