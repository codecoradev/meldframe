#!/bin/sh
# Usage: gate.sh <gate-name>. Decides whether a CI gate runs for real or is still pending, and
# writes `pending=true|false` to $GITHUB_OUTPUT (stdout when unset) for the steps that follow.
#
# A gate is pending only while its name is listed in .gitea/ci-pending.txt. The list is edited by
# hand: the pull request that makes a command exist (T0.1 workspace, T0.2 deps-check, T0.3 the
# other xtask commands) removes that command's line, and from then on the gate runs and fails
# like any other. A gate that is not listed is never skipped, so a missing command is a red check,
# not a silent pass.
set -eu
name="${1:?gate name}"
list="$(dirname "$0")/../../.gitea/ci-pending.txt"
out="${GITHUB_OUTPUT:-/dev/stdout}"
if [ -f "$list" ] && grep -Eq "^[[:space:]]*$name[[:space:]]*(#.*)?\$" "$list"; then
  # Once the workspace exists, the three Cargo gates cannot stay pending by mistake.
  root="$(dirname "$0")/../.."
  case "$name" in
    fmt | clippy | test)
      if [ -f "$root/Cargo.toml" ]; then
        echo "::error::Gate '$name' is still listed in .gitea/ci-pending.txt but a Cargo workspace exists. Remove the line."
        exit 1
      fi ;;
  esac
  echo "::notice::Gate '$name' is pending: its command does not exist yet (see .gitea/ci-pending.txt). It reports success without running until that line is removed."
  echo "pending=true" >> "$out"
else
  echo "pending=false" >> "$out"
fi
