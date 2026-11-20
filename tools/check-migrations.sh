#!/usr/bin/env bash
# Migration check: apply every file in migrations/ in order to a brand-new empty database and assert the schema
# contract (schema version on every stored file, op and snapshot; append-only ops). Needs a Postgres server.
#   ADMIN_DATABASE_URL  connection to any database on the server, used to create and drop the scratch database
#                       (default postgres://postgres@127.0.0.1:5433/postgres, see scripts/pg-start.sh).
set -euo pipefail
cd "$(dirname "$0")/.."

ADMIN_URL=${ADMIN_DATABASE_URL:-postgres://postgres@127.0.0.1:5433/postgres}
DB="meldframe_migrate_$$"
# Same host and credentials as the admin URL, other database name.
URL="${ADMIN_URL%/*}/$DB"
VERSION=$(grep -oE 'SCHEMA_VERSION: u32 = [0-9]+' crates/core/meldframe-protocol/src/version.rs | grep -oE '[0-9]+$')
[ -n "$VERSION" ] || { echo "FAIL: cannot read SCHEMA_VERSION from the protocol crate" >&2; exit 1; }

psql "$ADMIN_URL" -q -v ON_ERROR_STOP=1 -c "create database $DB"
trap 'psql "$ADMIN_URL" -q -c "drop database if exists $DB" >/dev/null 2>&1 || true' EXIT

echo "[1/4] migrations apply to an empty database"
shopt -s nullglob
FILES=(migrations/*.sql)
[ ${#FILES[@]} -gt 0 ] || { echo "FAIL: no migrations found" >&2; exit 1; }
for f in "${FILES[@]}"; do
  echo "  $f"
  psql "$URL" -q -v ON_ERROR_STOP=1 -f "$f" >/dev/null
done

echo "[2/4] every stored file, op and snapshot carries a schema version"
psql "$URL" -q -v ON_ERROR_STOP=1 -v v="$VERSION" <<'SQL'
do $$
declare t text;
begin
  foreach t in array array['files', 'ops', 'snapshots'] loop
    if not exists (
      select 1 from information_schema.columns
      where table_name = t and column_name = 'schema_version' and is_nullable = 'NO' and data_type = 'integer'
    ) then
      raise exception 'table % has no NOT NULL integer schema_version column', t;
    end if;
  end loop;
end $$;

-- The version the protocol crate declares is accepted.
insert into files (id, schema_version, workspace_id, name) values ('f1', :v, 'ws', 'f1');
insert into ops (file_id, seq, idempotency_key, actor, command, schema_version, bytes)
  values ('f1', 1, 'k1', 'a1', '{"type":"removeNode","id":"n"}', :v, 0);
insert into snapshots (file_id, seq, state, schema_version) values ('f1', 1, '{}', :v);

-- A missing or non-positive version is refused.
do $$
begin
  begin insert into files (id, workspace_id, name) values ('f2', 'ws', 'f2'); raise exception 'files accepted a missing schema_version';
  exception when not_null_violation then null; end;
  begin insert into files (id, schema_version, workspace_id, name) values ('f2', 0, 'ws', 'f2'); raise exception 'files accepted version 0';
  exception when check_violation then null; end;
  begin
    insert into ops (file_id, seq, idempotency_key, actor, command, schema_version, bytes) values ('f1', 2, 'k2', 'a', '{}', 0, 0);
    raise exception 'ops accepted version 0';
  exception when check_violation then null; end;
  begin insert into snapshots (file_id, seq, state, schema_version) values ('f1', 2, '{}', 0);
    raise exception 'snapshots accepted version 0';
  exception when check_violation then null; end;
end $$;
SQL

echo "[3/4] ops and calls are append-only; ops are idempotent per actor and file"
psql "$URL" -q -v ON_ERROR_STOP=1 <<'SQL'
do $$
begin
  insert into calls (at_ms, workspace_id, actor, tool, file, risk, decision, outcome)
    values (1, 'ws', 'a1', 'paint', 'f1', 'change', 'allow', 'ok');
  begin update calls set outcome = 'x';
    raise exception 'a call was updated';
  exception when others then
    if sqlerrm not like 'calls are append-only%' then raise; end if;
  end;
  begin insert into calls (at_ms, workspace_id, actor, tool, file, risk, decision, outcome)
      values (1, 'ws', 'a1', 'paint', 'f1', 'huge', 'allow', 'ok');
    raise exception 'an unknown risk was accepted';
  exception when check_violation then null; end;
end $$;
SQL
psql "$URL" -q -v ON_ERROR_STOP=1 <<'SQL'
do $$
begin
  begin update ops set actor = 'b' where file_id = 'f1' and seq = 1;
    raise exception 'an op was updated';
  exception when others then
    if sqlerrm not like 'ops are append-only%' then raise; end if;
  end;
  begin insert into ops (file_id, seq, idempotency_key, actor, command, schema_version, bytes)
      values ('f1', 3, 'k1', 'a1', '{}', 1, 0);
    raise exception 'a repeated idempotency key was accepted';
  exception when unique_violation then null; end;
  begin insert into ops (file_id, seq, idempotency_key, actor, command, schema_version, bytes)
      values ('f1', 1, 'k9', 'a1', '{}', 1, 0);
    raise exception 'a repeated seq was accepted';
  exception when unique_violation then null; end;
  begin insert into ops (file_id, seq, idempotency_key, actor, command, schema_version, bytes)
      values ('missing', 1, 'k1', 'a1', '{}', 1, 0);
    raise exception 'an op for an unknown file was accepted';
  exception when foreign_key_violation then null; end;
  -- The key is unique per actor within a file (the Store seam's contract): another actor may reuse it.
  insert into ops (file_id, seq, idempotency_key, actor, command, schema_version, bytes)
    values ('f1', 4, 'k1', 'a2', '{}', 1, 0);
  begin insert into ops (file_id, seq, idempotency_key, actor, command, schema_version, bytes)
      values ('f1', 5, 'k1', 'a2', '{}', 1, 0);
    raise exception 'a repeated key of the same actor was accepted';
  exception when unique_violation then null; end;
  -- Deleting a file removes its history with it.
  delete from files where id = 'f1';
  if exists (select 1 from ops) or exists (select 1 from snapshots) then raise exception 'history outlived its file'; end if;
end $$;
SQL
echo "[4/4] batch headers (ADR-0021): bounds of a row, what an update may change, and the cascade with the file"
psql "$URL" -q -v ON_ERROR_STOP=1 -v v="$VERSION" <<'SQL'
do $$
begin
  insert into files (id, schema_version, workspace_id, name) values ('f3', 1, 'ws', 'f3');
  insert into batch_files (file_id, workspace_id) values ('f3', 'ws');
  insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
    values ('f3', 'ws', 1, 'agent:a', 'k1', 'apply_commands', 5, 3, true, 1, 1, 3, 10);
  if (select review from batches where file_id = 'f3') <> 'unreviewed' then raise exception 'a header is not unreviewed by default'; end if;
  -- The bounds of one row.
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, note, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', 'k2', 't', 1, 1, true, repeat('x', 501), 2, 1, 1, 1);
    raise exception 'a note over 500 characters was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, review, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', 'k2', 't', 1, 1, true, 'rejected', 2, 1, 1, 1);
    raise exception 'a review state outside the three was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 0, 'a', 'k2', 't', 1, 1, true, 2, 1, 1, 1);
    raise exception 'batch number 0 was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', 'k2', 't', 100001, 1, true, 2, 1, 1, 1);
    raise exception 'more than 100000 commands were accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', '', 't', 1, 1, true, 2, 1, 1, 1);
    raise exception 'an empty key prefix was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 1, 'a', 'k2', 't', 1, 1, true, 2, 1, 1, 1);
    raise exception 'a repeated number was accepted';
  exception when unique_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'agent:a', 'k1', 't', 1, 1, true, 2, 1, 1, 1);
    raise exception 'a repeated actor and key prefix was accepted';
  exception when unique_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('missing', 'ws', 1, 'a', 'k', 't', 1, 1, true, 1, 1, 1, 1);
    raise exception 'a header for an unknown file was accepted';
  exception when foreign_key_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, review, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', 'k2', 't', 1, 1, true, 'accepted', 2, 1, 1, 1);
    raise exception 'an accepted header with no one who accepted it was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', 'k2', 't', 1, 2, true, 2, 1, 1, 1);
    raise exception 'more written than commands was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', 'k2', 't', 1, 1, true, 2, 5, 4, 1);
    raise exception 'first_seq above last_seq was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, repeat('a', 129), 'k2', 't', 1, 1, true, 2, 1, 1, 1);
    raise exception 'an actor over 128 bytes was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', 'k2', repeat('t', 65), 1, 1, true, 2, 1, 1, 1);
    raise exception 'a tool over 64 bytes was accepted';
  exception when check_violation then null; end;
  begin insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, review, reviewed_by, reviewed_at_ms, rev, first_seq, last_seq, at_ms)
      values ('f3', 'ws', 2, 'a', 'k2', 't', 1, 1, true, 'accepted', repeat('p', 129), 1, 2, 1, 1, 1);
    raise exception 'a reviewer over 128 bytes was accepted';
  exception when check_violation then null; end;
end $$;

-- What an update may change: written and commands only upward, last_seq only upward, a note only from null, rev only
-- upward, and a review with who and when. Everything else, and every change without a higher rev, is refused.
do $$

begin
  update batches set written = 4, commands = 6, last_seq = 4, rev = 2 where file_id = 'f3' and n = 1;
  update batches set note = 'first', rev = 3 where file_id = 'f3' and n = 1;
  update batches set review = 'accepted', reviewed_by = 'person:p', reviewed_at_ms = 11, rev = 4 where file_id = 'f3' and n = 1;
  update batches set review = 'unreviewed', reviewed_by = 'person:q', reviewed_at_ms = 12, rev = 5 where file_id = 'f3' and n = 1;
  perform 1 from batches where file_id = 'f3' and n = 1 and (written, commands, last_seq, note, rev) = (4, 6, 4, 'first', 5);
  if not found then raise exception 'the allowed updates did not take'; end if;
  begin update batches set written = 3, rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'written was lowered';
  exception when others then if sqlerrm not like 'a batch header only rises%' then raise; end if; end;
  begin update batches set written = 7, rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'written was raised above commands';
  exception when check_violation then null; end;
  begin update batches set commands = 5, rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'commands was lowered';
  exception when others then if sqlerrm not like 'a batch header only rises%' then raise; end if; end;
  begin update batches set last_seq = 3, rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'last_seq was lowered';
  exception when others then if sqlerrm not like 'a batch header only rises%' then raise; end if; end;
  begin update batches set rev = 4 where file_id = 'f3' and n = 1;
    raise exception 'rev was lowered';
  exception when others then if sqlerrm not like 'a batch header only rises%' then raise; end if; end;
  begin update batches set note = 'second', rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'a note was changed';
  exception when others then if sqlerrm not like 'a batch note is set once%' then raise; end if; end;
  begin update batches set note = null, rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'a note was removed';
  exception when others then if sqlerrm not like 'a batch note is set once%' then raise; end if; end;
  begin update batches set actor = 'agent:b', rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'the actor was changed';
  exception when others then if sqlerrm not like 'a batch header keeps its identity%' then raise; end if; end;
  begin update batches set key_prefix = 'k9', rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'the key prefix was changed';
  exception when others then if sqlerrm not like 'a batch header keeps its identity%' then raise; end if; end;
  begin update batches set n = 9 where file_id = 'f3' and n = 1;
    raise exception 'the number was changed';
  exception when others then if sqlerrm not like 'a batch header keeps its identity%' then raise; end if; end;
  begin update batches set first_seq = 0, rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'first_seq was changed';
  exception when others then if sqlerrm not like 'a batch header keeps its identity%' then raise; end if; end;
  begin update batches set at_ms = 99, rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'the time was changed';
  exception when others then if sqlerrm not like 'a batch header keeps its identity%' then raise; end if; end;
  insert into batches (file_id, workspace_id, n, actor, key_prefix, tool, commands, written, atomic, rev, first_seq, last_seq, at_ms)
    values ('f3', 'ws', 2, 'agent:a', 'k2', 'apply_commands', 1, 1, true, 6, 4, 4, 20);
  begin update batches set review = 'needs_review', rev = 7 where file_id = 'f3' and n = 2;
    raise exception 'a review changed without who and when';
  exception when others then if sqlerrm not like 'a review is set together%' then raise; end if; end;
  begin update batches set review = 'needs_review', reviewed_by = null, reviewed_at_ms = null, rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'a review changed with who and when removed';
  exception when others then if sqlerrm not like 'a review is set together%' then raise; end if; end;
  begin update batches set written = 5 where file_id = 'f3' and n = 1;
    raise exception 'a change took no new rev';
  exception when others then if sqlerrm not like 'a change of a batch header takes the next rev%' then raise; end if; end;
  begin update batches set review = 'rejected', rev = 6 where file_id = 'f3' and n = 1;
    raise exception 'a review state outside the three was set';
  exception when check_violation then null; end;
  -- A note over 500 characters on a header that has none (a stored note cannot be changed at all).
  begin update batches set note = repeat('x', 501), rev = 7 where file_id = 'f3' and n = 2;
    raise exception 'a note over 500 characters was set';
  exception when check_violation then null; end;
  -- The counter row only rises in next_n and next_rev, and open headers stay within the headers.
  update batch_files set next_n = 3, next_rev = 9 where file_id = 'f3';
  begin update batch_files set next_n = 2 where file_id = 'f3';
    raise exception 'next_n was lowered';
  exception when others then if sqlerrm not like 'a batch counter only rises%' then raise; end if; end;
  begin update batch_files set next_rev = 8 where file_id = 'f3';
    raise exception 'next_rev was lowered';
  exception when others then if sqlerrm not like 'a batch counter only rises%' then raise; end if; end;
  begin update batch_files set open_headers = 5 where file_id = 'f3';
    raise exception 'more open headers than headers were accepted';
  exception when check_violation then null; end;
  begin update batch_files set workspace_id = 'other' where file_id = 'f3';
    raise exception 'a counter row changed its workspace';
  exception when others then if sqlerrm not like 'a batch counter row keeps its file%' then raise; end if; end;
  -- Deleting a file removes its headers and their counters with it, so a file name that is recreated starts again.
  delete from files where id = 'f3';
  if exists (select 1 from batches) or exists (select 1 from batch_files) then raise exception 'headers outlived their file'; end if;
end $$;
SQL
echo "OK: migrations apply from an empty database and hold the schema contract"
