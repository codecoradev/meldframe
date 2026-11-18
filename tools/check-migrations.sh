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

echo "[1/3] migrations apply to an empty database"
shopt -s nullglob
FILES=(migrations/*.sql)
[ ${#FILES[@]} -gt 0 ] || { echo "FAIL: no migrations found" >&2; exit 1; }
for f in "${FILES[@]}"; do
  echo "  $f"
  psql "$URL" -q -v ON_ERROR_STOP=1 -f "$f" >/dev/null
done

echo "[2/3] every stored file, op and snapshot carries a schema version"
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

echo "[3/3] ops and calls are append-only; ops are idempotent per actor and file"
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
echo "OK: migrations apply from an empty database and hold the schema contract"
