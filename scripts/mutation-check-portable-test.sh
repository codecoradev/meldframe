#!/usr/bin/env bash
# Guards tools/mutation-check-sync.sh against what broke it on a stock Mac (issue 233): bash 3.2 does not parse a here-document inside
# `$( ... )` when the table has unbalanced parentheses or quotes, and BSD sed takes the first argument after `-i` as the backup
# suffix, so `sed -i "s#a#b#" file` reads the script as the suffix and the file as a command ("command c expects \ followed by
# text"). Two guards, neither of them a Mac: a search of the script for those two constructs, and a run of a copy in a scratch git
# repository with a `sed` shim that models ONLY that `-i` difference (it fails on a bare -i and is GNU sed otherwise, so the 71 table
# patterns are not run under a BSD regex dialect) and a fake `cargo` whose tests fail, so a mutant is killed as soon as its pattern has
# been applied. The copy runs under /bin/bash when that is bash 3.2 (a Mac), else under the bash on the PATH, which parses the old
# table too: on Linux only the search catches that construct, and it catches only the one-line spelling `$(cat <<` (not a here-document
# opened on the next line, in backticks or after another command). Run by `just check`.
set -u
cd "$(dirname "$0")/.."
root=$(pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0
ok() { echo "pass  $1"; }
bad() { echo "FAIL  $1"; shift; [ $# -gt 0 ] && { printf '%s\n' "$@" | sed 's/^/      /'; }; fail=1; }
script=tools/mutation-check-sync.sh

# 1. The two constructs bash 3.2 and BSD sed cannot take, searched for in the script (full-line comments left out).
code=$(grep -vE '^[[:space:]]*#' "$script")
if printf '%s\n' "$code" | grep -qE '\$\([[:space:]]*cat[[:space:]]*<<'; then
  bad "a here-document inside \$( ) (bash 3.2 does not parse a table with unbalanced quotes there): $script"
else ok "no here-document inside \$( )"; fi
if printf '%s\n' "$code" | grep -qE '(^|[^A-Za-z0-9_./-])sed[[:space:]]+([^|;&)]*[[:space:]])?(-[A-Za-z]*i[A-Za-z]*(["'"'"']{2})?([[:space:]]|$)|--in-place)'; then
  bad "sed -i with no suffix attached (BSD sed reads the next word as the suffix): $script"
else ok "no sed -i without a suffix attached"; fi

# 2. A run of a copy in a scratch repository: the table is read (--list), and one mutant is applied with the BSD-like sed and killed.
mkdir -p "$tmp/repo/tools" "$tmp/bin"
cp "$script" "$tmp/repo/tools/"
bash_cmd=bash
if [ -x /bin/bash ] && /bin/bash --version | head -n 1 | grep -q 'version 3\.'; then bash_cmd=/bin/bash; fi
export TMPDIR="$tmp" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
first=$(awk 'f{print; exit} /<<.TABLE.( \|\| true)?$/{f=1}' "$script")  # the first line of the table
name=$(printf '%s\n' "$first" | cut -f1)
file=$(printf '%s\n' "$first" | cut -f2)
mkdir -p "$tmp/repo/$(dirname "$file")"
cp "$file" "$tmp/repo/$file"
if ! { git -C "$tmp/repo" init -q && git -C "$tmp/repo" add -A && git -C "$tmp/repo" -c user.name=t -c user.email=t@example.com -c commit.gpgsign=false commit -q --no-verify -m scratch; }; then
  bad "the scratch repository could not be set up"; exit 1
fi

cat >"$tmp/bin/sed" <<SED
#!/bin/sh
# BSD sed about -i: the word after a bare -i is the backup suffix, so the script becomes the suffix and the file a command.
for a in "\$@"; do [ "\$a" = "-i" ] && { echo "sed: 1: \"\$3\": command c expects \\\\ followed by text" >&2; exit 1; }; done
exec $(command -v sed) "\$@"
SED
cat >"$tmp/bin/cargo" <<'CARGO'
#!/bin/sh
# --no-run builds fine; a test run fails with one failing test, as a killed mutant does.
for a in "$@"; do [ "$a" = "--no-run" ] && exit 0; done
echo "test fake::a_test ... FAILED"
exit 101
CARGO
chmod +x "$tmp/bin/sed" "$tmp/bin/cargo"

listed=$(PATH="$tmp/bin:$PATH" "$bash_cmd" "$tmp/repo/$script" --list 2>&1)
count=$(printf '%s\n' "$listed" | grep -c .)
if [ "$count" -ge 50 ] && printf '%s\n' "$listed" | grep -qx "$name"; then ok "--list prints the table ($count mutants, first: $name)"; else bad "--list did not print the table" "$listed"; fi

out=$(cd "$tmp/repo" && PATH="$tmp/bin:$PATH" "$bash_cmd" "tools/mutation-check-sync.sh" "$name" 2>&1)
status=$?
if [ "$status" -eq 0 ] && printf '%s\n' "$out" | grep -q "^killed:   $name" && printf '%s\n' "$out" | grep -q "1 killed, 0 survived"; then
  ok "a mutant is applied with a BSD-like sed and killed ($name)"
else
  bad "the run of $name did not end killed (exit $status)" "$out"
fi
# A sed that fails: the run says so (not "pattern not found"), exits with 2 and leaves nothing behind.
printf '#!/bin/sh\necho "sed: boom" >&2\nexit 1\n' > "$tmp/bin/sed-fails"
mkdir -p "$tmp/failbin" && cp "$tmp/bin/cargo" "$tmp/failbin/cargo" && cp "$tmp/bin/sed-fails" "$tmp/failbin/sed" && chmod +x "$tmp/failbin/sed" "$tmp/failbin/cargo"
out=$(cd "$tmp/repo" && PATH="$tmp/failbin:$PATH" "$bash_cmd" "tools/mutation-check-sync.sh" "$name" 2>&1)
status=$?
if [ "$status" -eq 2 ] && printf '%s\n' "$out" | grep -q "FAIL: $name: sed failed on $file"; then ok "a failing sed is reported as such (exit 2)"; else bad "a failing sed was not reported as such (exit $status)" "$out"; fi
if git -C "$tmp/repo" diff --quiet && [ -z "$(git -C "$tmp/repo" status --porcelain)" ]; then ok "the file is restored and nothing is left behind"; else bad "the run left changes in the scratch repository" "$(git -C "$tmp/repo" status --short)"; fi
exit "$fail"
