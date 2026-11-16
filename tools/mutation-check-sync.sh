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
cascade	crates/core/meldframe-sync/src/replica/rebuild.rs	let alive = !p\.out\.deps\.iter()\.any(|d| dead\.contains(d))	let alive = true	a refused op does not take its dependents with it
per-actor-undo	crates/core/meldframe-undo/src/remote.rs	drop_fields(&mut patch, rp);	let _ = rp;	undo keeps its old value for a field another actor wrote after me
put-back	crates/core/meldframe-undo/src/runs.rs	if restore && alive && m\.shadow\.strikes < 2 {	if false {	a refused undo or redo does not put the history step back
snapshot-patch	crates/core/meldframe-undo/src/remote.rs	node\.apply_patch(rp)	()	a remote edit is not written into the node my undo will add again
replay-undo	crates/core/meldframe-undo/src/history.rs	r\.undo_ = undo\.clone();	let _ = \&undo;	a replayed pending op keeps a stale undo
pending-skip	crates/core/meldframe-undo/src/remote.rs	|r: &&mut Rec| r\.pending || held\.contains(&r\.id)	|r: \&\&mut Rec| false || held.contains(\&r.id)	a pending rec is rewritten against a remote op that is ordered before it
seq-guard	crates/core/meldframe-sync/src/replica.rs	if seq <= self.seq {	if false \&\& seq <= self.seq {	a duplicate server event is not ignored
reparent-survivor	crates/core/meldframe-undo/src/remote.rs	let parent_id = ctx\.survivor\.clone();	let parent_id: Option<String> = None;	undo of a move into a deleted parent goes to the top instead of the nearest surviving ancestor
held-recs	crates/core/meldframe-undo/src/runs.rs	let hit = |r: &Rec| r\.pending || held\.contains(&r\.id);	let hit = |r: \&Rec| false \&\& r.pending;	the step an undo or redo made from a pending edit is rewritten against a remote op that is ordered before the edit (the redo loses its field)
refresh-run	crates/core/meldframe-undo/src/runs.rs	if self\.held_recs()\.contains(&rec) {	if false {	the step an undo or redo made keeps the stale value its op carried
correction	crates/core/meldframe-sync/src/replica/rebuild.rs	Some(w) => self\.correct(&w, &lost),	Some(w) => { let _ = \&w; },	a stale undo or redo that is already on its way is not set right
strip-remote	crates/core/meldframe-sync/src/replica/rebuild.rs	p\.intent = p\.intent\.take()\.and_then(|i| without_fields(&i, r));	let _ = r;	a correction is kept for a field another actor wrote after it was made
want-undo	crates/core/meldframe-sync/src/replica/rebuild.rs	self\.view\.set_undo(r, own\.unwrap_or(inverse));	self.view.set_undo(r, inverse);	the undo of an edit made after a stale undo is worked out in the stale view
refused-correction	crates/core/meldframe-sync/src/replica.rs	!p\.is_correction(),	true,	a refused correction is sent again without end
ideal-persist	crates/core/meldframe-sync/src/replica/rebuild.rs	self\.ideal = Some(w);	drop(w);	a stale undo that the server applied is forgotten while a later edit is still pending
ideal-reset	crates/core/meldframe-sync/src/replica/rebuild.rs	if dropped > 0 || self\.pending\.is_empty() {	if self.pending.is_empty() {	a refused or dropped op leaves ideal and the correction with what its step would have added
refused-resend	crates/core/meldframe-sync/src/replica.rs	(p\.is_correction() && !p\.resent)\.then(	(false \&\& p.is_correction() \&\& !p.resent).then(	a refused correction is not sent once more, so the stale value stays in the confirmed state
resent-flag	crates/core/meldframe-sync/src/replica/rebuild.rs	last\.resent = resent;	let _ = resent;	a correction that was sent once more is sent again after its refusal, without end
lost-base	crates/core/meldframe-sync/src/replica/rebuild.rs	\.filter_map(|c| left_in_base(&self\.base, self\.view\.nodes(), &pend, c))	.filter_map(|_| None::<Command>)	a refused correction whose dependent ops fell is not sent once more
lost-cascade	crates/core/meldframe-sync/src/replica/rebuild.rs	if chain && p\.is_correction() && !p\.resent {	if false \&\& chain \&\& p.is_correction() \&\& !p.resent {	a correction that falls with a refused correction is not sent once more
pending-written	crates/core/meldframe-sync/src/correction.rs	!written\.contains(k) && 	true \&\&	a resend overwrites a pending edit of the field that holds the same value as the stale one
own-by-key	crates/core/meldframe-sync/src/replica.rs	Some(k) => p\.out\.key == k,	Some(_) => p.json == cmd_json(cmd),	an event of my actor is taken for the ack of my oldest pending op by actor and command, not by key
own-keyless	crates/core/meldframe-sync/src/replica.rs	None => p\.json == cmd_json(cmd),	None => false,	an event without a key (an old log) is never recognised as mine
resync-seq	crates/core/meldframe-sync/src/replica/resync.rs	self\.seq = seq;	let _ = seq;	a resync does not move the replica to the sequence number of the snapshot
resync-applied	crates/core/meldframe-sync/src/replica/resync.rs	if done\.contains(p\.out\.key\.as_str()) {	if false \&\& done.contains(p.out.key.as_str()) {	an op the snapshot already holds is replayed (and sent) again after a resync
resync-deps	crates/core/meldframe-sync/src/replica/resync.rs	let live = !p\.out\.deps\.iter()\.any(|d| dead\.contains(d))	let live = true	after a resync an op whose dependency was dropped is kept
resync-outbox	crates/core/meldframe-sync/src/replica/resync.rs	self\.outbox = self\.pending\.iter()\.map(|p| p\.out\.clone())\.collect();	self.outbox.clear();	a resync does not queue the pending ops to be sent again
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
  if ! cargo test -p meldframe-history -p meldframe-sync -p meldframe-sync-scenarios -p meldframe-sync-props --no-run >"/tmp/mutation-$name.build.log" 2>&1; then
    git checkout -- "$file"
    current=""
    echo "BROKEN:   $name does not build (see /tmp/mutation-$name.build.log)"
    exit 2
  fi
  cargo test -p meldframe-history -p meldframe-sync -p meldframe-sync-scenarios -p meldframe-sync-props --no-fail-fast >"/tmp/mutation-$name.log" 2>&1
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
