#!/usr/bin/env bash
# Mutation check for the replica (E1.1): switch one rule off in the source, run the tests and demand
# that at least one fails, then restore the file. A mutant that survives is reported and fails the run.
#
#   tools/mutation-check-sync.sh            run every mutation
#   tools/mutation-check-sync.sh NAME ...   run the named ones
#   tools/mutation-check-sync.sh --list     print the names
#
# Each mutant runs in two passes. The quick pass stops at the first failing test binary and runs the
# property tests with MELDFRAME_MUTATION_SEEDS seeds (100 by default); most mutants die there in seconds.
# A mutant that survives it gets the full pass (every test, MELDFRAME_SEEDS seeds, 600 by default), so a
# mutant is only reported as surviving when the full suite misses it. The working tree must be clean in
# the files a mutation touches; they are restored with `git checkout`.
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
correction	crates/core/meldframe-sync/src/replica/rebuild.rs	Some(w) => self\.correct(&w, &lost, &touched),	Some(w) => { let _ = (\&w, \&touched); },	a stale undo or redo that is already on its way is not set right
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
cap-check	crates/core/meldframe-sync/src/replica.rs	self\.check_room()?;	let _ = self.check_room();	a local op, undo or redo is accepted although the queue of pending ops is full
cap-edge	crates/core/meldframe-sync/src/replica.rs	match self\.pending\.len() >= self\.max_pending {	match self.pending.len() > self.max_pending {	the cap lets one op more than the maximum in
resync-seq	crates/core/meldframe-sync/src/replica/resync.rs	self\.seq = seq;	let _ = seq;	a resync does not move the replica to the sequence number of the snapshot
resync-applied	crates/core/meldframe-sync/src/replica/resync.rs	if done\.contains(p\.out\.key\.as_str()) {	if false \&\& done.contains(p.out.key.as_str()) {	an op the snapshot already holds is replayed (and sent) again after a resync
resync-deps	crates/core/meldframe-sync/src/replica/resync.rs	let live = !p\.out\.deps\.iter()\.any(|d| dead\.contains(d))	let live = true	after a resync an op whose dependency was dropped is kept
resync-outbox	crates/core/meldframe-sync/src/replica/resync.rs	self\.outbox = self\.pending\.iter()\.map(|p| p\.out\.clone())\.collect();	self.outbox.clear();	a resync does not queue the pending ops to be sent again
app-key	crates/app/meldframe-app/src/app.rs	\.on_event_keyed(seq, actor, key, command)	.on_event_keyed(seq, actor, None, command)	the app ignores the key an event frame carries and compares actor and command
app-reload-applied	crates/app/meldframe-app/src/link.rs	self\.replica\.resync(nodes, seq, applied)	self.replica.resync(nodes, seq, \&[])	a reload replays ops the snapshot already holds
app-reload-subscribe	crates/app/meldframe-app/src/link.rs	self\.start(now_ms, fx);	let _ = now_ms;	a reload does not go to connecting and subscribe from the snapshot
app-reload-halted	crates/app/meldframe-app/src/app.rs	!ui \&\& !reload {	!ui {	a halted app ignores a reload like any other action
app-room	crates/app/meldframe-app/src/document.rs	if self\.replica\.pending_len() + cmds\.len() > max {	if false {	a list action that does not fit the queue is applied in part
own-by-key	crates/core/meldframe-sync/src/replica.rs	Some(k) => self\.pending\.iter()\.position(|p| p\.out\.key == k),	Some(_) => (self.pending.front().map(|p| \&p.json) == Some(\&cmd_json(cmd))).then_some(0),	an event of my actor is taken for the ack of my oldest pending op by actor and command, not by key
own-keyless	crates/core/meldframe-sync/src/replica.rs	None => (self\.pending\.front()\.map(|p| \&p\.json) == Some(\&cmd_json(cmd)))\.then_some(0),	None => None,	an event without a key (an old log) is never recognised as mine
own-out-of-order	crates/core/meldframe-sync/src/replica.rs	Some(k) => self\.pending\.iter()\.position(|p| p\.out\.key == k),	Some(k) => self.pending.front().filter(|p| p.out.key == k).map(|_| 0),	an op of mine that the server ordered ahead of an older one is not confirmed by its key
own-diverged	crates/core/meldframe-sync/src/replica.rs	if self\.pending\[i\]\.json != cmd_json(cmd) {	if false {	a key of my pending op that comes back with another command is taken for my op
own-actor-guard	crates/core/meldframe-sync/src/replica.rs	if let Some(i) = at\.filter(|_| actor == self\.actor) {	if let Some(i) = at {	another actor that uses the key of my pending op is taken for me
ooo-settle	crates/core/meldframe-sync/src/replica/confirm.rs	self\.settle(i);	self.pending.remove(i);	an op confirmed by its event keeps its history step pending
ack-guard	crates/core/meldframe-sync/src/replica/confirm.rs	if seq > self\.seq {	if false {	an answer that runs ahead of the stream drains an op whose event is still to come
resync-behind	crates/core/meldframe-sync/src/replica/resync.rs	if seq < self\.seq {	if false {	a snapshot older than the replica is accepted
resync-kind	crates/core/meldframe-sync/src/replica/resync.rs	if p\.out\.kind != OpKind::Edit {	if false {	a replayed undo or redo op is not kept as an op that sets its value (the replica then corrects it)
resync-optimistic	crates/core/meldframe-sync/src/replica/resync.rs	view\.set_optimistic(true);	view.set_optimistic(false);	the view after a resync is not optimistic
resync-ideal	crates/core/meldframe-sync/src/replica/resync.rs	self\.drop_ideal();	{}	a resync keeps the confirmed state a stale undo was worked out against
undo-empty	crates/core/meldframe-sync/src/replica.rs	if !self\.view\.can_undo() {	if false {	undo at the cap with nothing to undo is an error, not false
redo-empty	crates/core/meldframe-sync/src/replica.rs	if !self\.view\.can_redo() {	if false {	redo at the cap with nothing to redo is an error, not false
app-acked	crates/app/meldframe-app/src/app.rs	if let Some(dropped) = self\.replica\.on_ack(&key, seq) {	if let Some(dropped) = None::<usize> {	the app ignores the answer to a repeated send, so an op the snapshot holds stays pending
app-dropped	crates/app/meldframe-app/src/app.rs	if count > 0 {	if false {	a reload that drops ops says nothing
app-refused-code	crates/app/meldframe-app/src/link.rs	code: e\.code()\.to_string(),	code: String::new(),	a refused reload does not say why
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
crates=(-p meldframe-history -p meldframe-sync -p meldframe-sync-scenarios -p meldframe-sync-props -p meldframe-app -p meldframe-app-tests)
while IFS=$'\t' read -r name file from to what; do
  if [ ${#want[@]} -gt 0 ] && [[ ! " ${want[*]} " == *" $name "* ]]; then continue; fi
  if ! git diff --quiet -- "$file"; then echo "FAIL: $file has local changes; commit or stash them first"; exit 2; fi
  current=$file
  sed -i "s#$from#$to#" "$file"
  if git diff --quiet -- "$file"; then echo "FAIL: $name: the pattern was not found in $file"; exit 2; fi
  # A mutant that does not compile, or a cargo failure unrelated to it, is not a kill: build first.
  if ! cargo test "${crates[@]}" --no-run >"/tmp/mutation-$name.build.log" 2>&1; then
    git checkout -- "$file"
    current=""
    echo "BROKEN:   $name does not build (see /tmp/mutation-$name.build.log)"
    exit 2
  fi
  MELDFRAME_SEEDS=${MELDFRAME_MUTATION_SEEDS:-100} cargo test "${crates[@]}" >"/tmp/mutation-$name.log" 2>&1
  status=$?
  # The full pass runs when the quick pass found nothing, and also when it stopped without a failing test (a test
  # binary that crashed hides the binaries after it), so BROKEN is only decided on the full suite.
  if [ $status -eq 0 ] || ! grep -qE '^test [A-Za-z0-9_:]+ \.\.\. FAILED$' "/tmp/mutation-$name.log"; then
    cargo test "${crates[@]}" --no-fail-fast >"/tmp/mutation-$name.log" 2>&1
    status=$?
  fi
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
