#!/usr/bin/env bash
# Mutation check for the replica (E1.1): switch one rule off in the source, run the tests and demand
# that at least one fails, then restore the file. A mutant that survives is reported and fails the run.
#
#   tools/mutation-check-sync.sh            run every mutation
#   tools/mutation-check-sync.sh NAME ...   run the named ones
#   tools/mutation-check-sync.sh --list     print the names
#
# Each run takes about a minute (a build and 600 property-test seeds). The working tree must be clean
# in the files a mutation touches; they are restored with `git checkout`.
#
# Every mutation is one line of tab-separated fields: name, file, sed pattern (# is the delimiter),
# replacement, and what it switches off.
set -uo pipefail
cd "$(dirname "$0")/.."
export CARGO_INCREMENTAL=0

MUTATIONS=$(cat <<'TABLE'
deps-client	crates/core/meldframe-sync/src/replica.rs	if kind == OpKind::Edit {	if true {	the replica never names dependencies on an undo or redo
deps-server	crates/core/meldframe-sync-props/tests/sim/server.rs	.any(|d| !self\.idem\.contains_key(&(actor\.to_string(), d\.clone())))	.any(|_| false)	the server accepts an op although a dependency was not applied
cascade	crates/core/meldframe-sync/src/replica.rs	let alive = !p\.out\.deps\.iter()\.any(|d| dead\.contains(d))	let alive = true	a refused op does not take its dependents with it
per-actor-undo	crates/core/meldframe-history/src/remote.rs	drop_fields(&mut patch, rp);	let _ = rp;	undo keeps its old value for a field another actor wrote after me
put-back	crates/core/meldframe-history/src/runs.rs	if restore && alive && m\.shadow\.strikes < 2 {	if false {	a refused undo or redo does not put the history step back
snapshot-patch	crates/core/meldframe-history/src/remote.rs	node\.apply_patch(rp)	()	a remote edit is not written into the node my undo will add again
replay-undo	crates/core/meldframe-history/src/file_state.rs	r\.undo_ = inverse\.clone();	let _ = \&inverse;	a replayed pending op keeps a stale undo
pending-skip	crates/core/meldframe-history/src/remote.rs	\.filter(|r| !r\.pending)	.filter(|_| true)	a pending rec is rewritten against a remote op that is ordered before it
seq-guard	crates/core/meldframe-sync/src/replica.rs	if seq <= self.seq {	if false \&\& seq <= self.seq {	a duplicate server event is not ignored
TABLE
)

if [ "${1:-}" = "--list" ]; then
  cut -f1 <<<"$MUTATIONS"
  exit 0
fi

want=("$@")
current=""
trap '[ -n "$current" ] && git checkout -- "$current"' EXIT
killed=0; survived=0
while IFS=$'\t' read -r name file from to what; do
  if [ ${#want[@]} -gt 0 ] && [[ ! " ${want[*]} " == *" $name "* ]]; then continue; fi
  if ! git diff --quiet -- "$file"; then echo "FAIL: $file has local changes; commit or stash them first"; exit 2; fi
  current=$file
  sed -i "s#$from#$to#" "$file"
  if git diff --quiet -- "$file"; then echo "FAIL: $name: the pattern was not found in $file"; exit 2; fi
  # A mutant that does not compile, or a cargo failure unrelated to it, is not a kill: build first.
  if ! cargo test -p meldframe-history -p meldframe-sync -p meldframe-sync-props --no-run >"/tmp/mutation-$name.build.log" 2>&1; then
    git checkout -- "$file"
    current=""
    echo "BROKEN:   $name does not build (see /tmp/mutation-$name.build.log)"
    exit 2
  fi
  cargo test -p meldframe-history -p meldframe-sync -p meldframe-sync-props --no-fail-fast >"/tmp/mutation-$name.log" 2>&1
  status=$?
  git checkout -- "$file"
  current=""
  # Killed means at least one test really failed, not just a non-zero exit.
  if [ $status -ne 0 ] && grep -qE '^test [A-Za-z0-9_:]+ \.\.\. FAILED$' "/tmp/mutation-$name.log"; then
    killed=$((killed + 1))
    echo "killed:   $name ($what)"
    grep -E '^test [A-Za-z0-9_:]+ \.\.\. FAILED$' "/tmp/mutation-$name.log" | sed 's/^test /            /; s/ \.\.\. FAILED//' | head -3
  elif [ $status -ne 0 ]; then
    echo "BROKEN:   $name: the tests failed without a failing test (see /tmp/mutation-$name.log)"
    exit 2
  else
    survived=$((survived + 1))
    echo "SURVIVED: $name ($what)"
  fi
done <<<"$MUTATIONS"
echo "mutation check: $killed killed, $survived survived"
[ $survived -eq 0 ]
