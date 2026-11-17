#!/bin/sh
# Installs kache (a rustc wrapper that caches compiled crates by content) for the rest of a CI job.
# Pinned version and checked sha256 of the release archive; nothing is built from source.
# On any failure the job carries on without the wrapper (a cache is never a gate).
set -u
VERSION=2.1.1
case $(uname -m) in
  aarch64|arm64)
    ASSET=kache-aarch64-unknown-linux-musl.tar.gz
    SHA256=466f0aef4885f5291359b3c1bc0c9976824634194c5b67e1cee2a4fe3035155a ;;
  x86_64)
    ASSET=kache-x86_64-unknown-linux-musl.tar.gz
    SHA256=7603022ee78b48a9afde9055b0df27e5c2cef1351a54fa2de6b4fde0f37ae22c ;;
  *) echo "kache: no build for $(uname -m); building without it"; exit 0 ;;
esac
URL=https://github.com/kunobi-ninja/kache/releases/download/v$VERSION/$ASSET
DIR=${RUNNER_TEMP:-/tmp}/kache-bin
mkdir -p "$DIR" && cd "$DIR" || exit 0
if curl -fsSL --retry 3 -o "$ASSET" "$URL" && echo "$SHA256  $ASSET" | sha256sum -c - >/dev/null \
  && tar xzf "$ASSET" kache kache-engine && "$DIR/kache" --version >/dev/null 2>&1 && [ -x "$DIR/kache-engine" ]; then
  echo "$DIR" >> "${GITHUB_PATH:-/dev/null}"
  {
    echo "RUSTC_WRAPPER=$DIR/kache"
    echo "CARGO_INCREMENTAL=0"
    echo "KACHE_CACHE_DIR=$HOME/.cache/kache-store"
  } >> "${GITHUB_ENV:-/dev/null}"
  echo "kache $VERSION installed"
else
  echo "kache not installed (download, checksum or unpack failed); building without it"
fi
exit 0
