#!/bin/sh
# Usage: branch-name.sh <branch>. Exit 0 when the branch name is allowed, 1 when it is not.
# A branch starts with one of the prefixes below and a lowercase slug. `develop` and `main` are
# the long-lived branches and are allowed (a release pull request goes from develop to main).
set -eu
branch="${1:-}"
case "$branch" in
  develop | main) echo "ok: $branch (long-lived branch)"; exit 0 ;;
esac
if printf '%s\n' "$branch" | grep -Eq '^(feat|fix|docs|chore|refactor|perf|test|ci|build|release)/[a-z0-9][a-z0-9._-]*$'; then
  echo "ok: $branch"
else
  echo "::error::Branch '$branch' must be feat/, fix/, docs/, chore/, refactor/, perf/, test/, ci/, build/ or release/ followed by a lowercase slug (for example feat/room-snapshots)."
  exit 1
fi
