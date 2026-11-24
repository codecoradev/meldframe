#!/usr/bin/env bash
# Checks that the shell scripts of the repository keep to what a stock Mac has (issue 232: `shuf` is GNU coreutils and
# macOS does not ship it, so `just dev-smoke` stopped at its first line there). Two kinds of check, with nothing
# started: a scan of scripts/ and tools/ for tools and options that only GNU has, and a run of the port function of
# tools/dev-smoke.sh. Run by `just check`.
set -u
cd "$(dirname "$0")/.."
fail=0
ok() { echo "pass  $1"; }
bad() { echo "FAIL  $1"; shift; [ $# -gt 0 ] && { printf '%s\n' "$@" | sed 's/^/      /'; }; fail=1; }

# The scripts that are scanned: every shell script under scripts/ (CI checks included) and tools/. Full-line comments are
# not scanned, so a comment may name a tool (`# not shuf`). The list below is what has been hit so far, not every
# difference between GNU and BSD (`sed -i` without a suffix is another: tools/mutation-check-sync.sh has one, issue 233,
# and the pattern joins the list with its fix). Trailing comments and strings are scanned, so a line that only mentions a tool in
# a string is a false hit; write the mention in a full-line comment.
files=$(ls scripts/*.sh scripts/ci/*.sh tools/*.sh 2>/dev/null | grep -v '^scripts/portable-shell-test.sh$')
[ -n "$files" ] || { bad "no shell scripts found to scan (run from the repository)"; exit 1; }
opts='([^|;&)"'"'"']*[[:space:]])?' # other options between the command and the one that matters (not past a pipe, a ; or &, a ) or a quote)

scan() { # description, pattern for grep -E
  local hits
  # shellcheck disable=SC2086
  hits=$(grep -nE -- "$2" $files 2>/dev/null | grep -vE '^[^:]+:[0-9]+:[[:space:]]*#')
  if [ -z "$hits" ]; then ok "no $1"; else bad "$1 is not on a stock Mac" "$hits"; fi
}
# The patterns must catch the forms they are for: a line each must match.
catches() { # pattern, sample line
  if printf '%s\n' "$2" | grep -qE -- "$1"; then ok "the pattern catches: $2"; else bad "the pattern misses: $2"; fi
}

L='(^|[^A-Za-z0-9_./-])' # the command starts a word: `pgrep -P` and `lstat -c` are other commands
P_SHUF="${L}shuf([^A-Za-z_]|\$)"
P_READLINK="${L}readlink[[:space:]]+${opts}-[A-Za-z]*f"
P_DATE="${L}date[[:space:]]+${opts}(-[A-Za-z]*d([^A-Za-z]|\$)|--date)"
P_GREP_P="${L}grep[[:space:]]+${opts}(-[A-Za-z]*P|--perl-regexp)"
P_STAT="${L}stat[[:space:]]+${opts}(-[A-Za-z]*c([^A-Za-z]|\$)|--format|--printf)"
P_BASH4='(^|[^A-Za-z_])(mapfile|readarray)[[:space:]]|declare[[:space:]]+-A'
catches "$P_SHUF" 'PORT=$(shuf -i 1-9 -n 1)'
catches "$P_SHUF" 'seq 9 | shuf'
catches "$P_SHUF" 'x=$(cmd | shuf)'
catches "$P_READLINK" 'readlink -f "$0"'
catches "$P_READLINK" 'readlink -ef "$0"'
catches "$P_DATE" 'date -d yesterday +%F'
catches "$P_DATE" 'date -u -d @1 +%F'
catches "$P_DATE" 'date --date=yesterday'
catches "$P_DATE" 'date -d@1 +%F'
catches "$P_DATE" 'date -d"yesterday" +%F'
catches "$P_STAT" 'stat -c%s file'
catches "$P_STAT" 'stat --format=%s file'
misses() { # pattern, sample line that is fine
  if printf '%s\n' "$2" | grep -qE -- "$1"; then bad "the pattern flags a line that is fine: $2"; else ok "the pattern leaves alone: $2"; fi
}
misses "$P_GREP_P" 'pgrep -P $$'
misses "$P_DATE" 'echo "$(date +%s)" -d x'
misses "$P_READLINK" 'x=$(readlink "$f") -f'
misses "$P_DATE" 'echo update -d x'
misses "$P_STAT" 'lstat -c x'
catches "$P_GREP_P" "grep -P 'a+' f"
catches "$P_GREP_P" "grep -q -P 'a+' f"
catches "$P_GREP_P" "grep -E -P 'a' f"
catches "$P_STAT" 'stat -c %s file'
catches "$P_STAT" 'stat -L -c %s file'
catches "$P_BASH4" 'mapfile -t a < f'
catches "$P_BASH4" 'declare -A m'
scan "shuf (use \$RANDOM)" "$P_SHUF"
scan "readlink -f (macOS before 12.3 lacks it; use cd and pwd)" "$P_READLINK"
scan "date -d (BSD date means something else by -d)" "$P_DATE"
scan "grep -P (BSD grep has no -P)" "$P_GREP_P"
scan "stat -c (BSD stat has no -c)" "$P_STAT"
scan "mapfile, readarray or declare -A (bash 4)" "$P_BASH4"

# The port of tools/dev-smoke.sh: take the function out of the script and run it as the script would.
fn=$(grep -m1 '^pick_port()' tools/dev-smoke.sh)  # a one-line function
if [ -z "$fn" ]; then
  bad "tools/dev-smoke.sh has no pick_port function"
else
  # Called as the script calls it, in the shell that uses PORT, many times in a row (the first draw and the retry are two calls).
  out=$(bash -c "$fn"$'\n''for _ in $(seq 3000); do pick_port; echo "$PORT"; done' 2>&1)
  lo=$(printf '%s\n' "$out" | sort -n | head -n 1)
  hi=$(printf '%s\n' "$out" | sort -n | tail -n 1)
  distinct=$(printf '%s\n' "$out" | sort -u | wc -l | tr -d ' ')
  if printf '%s\n' "$out" | grep -qvE '^[0-9]+$'; then
    bad "pick_port prints something that is not a number" "$out"
  elif [ "$lo" -ge 20000 ] && [ "$hi" -le 60000 ] && [ "$lo" -le 20400 ] && [ "$hi" -ge 59600 ] && [ "$distinct" -ge 2000 ]; then
    ok "pick_port gives ports from 20000 to 60000 ($distinct distinct in 3000, $lo to $hi)"
  else
    bad "pick_port is out of range or hardly random ($distinct distinct in 3000, $lo to $hi)"
  fi
fi
exit "$fail"
