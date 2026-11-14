-- First migration: files, their append-only op log and snapshots.
--
-- Every stored file, op and snapshot carries `schema_version`, the version of the stored shape (see
-- `SCHEMA_VERSION` in meldframe-protocol), so a later change can migrate old data deliberately instead of guessing.
-- Workspace and tenancy tables arrive with their own epic (E2.3); nothing here references them yet.

-- A file is one design document. `seq` counts the ops accepted into its room, giving them one total order.
create table files (
  id             text primary key,
  schema_version integer not null check (schema_version >= 1),
  seq            bigint not null default 0,
  created_at     timestamptz not null default now()
);

-- Op log: the source of truth. Append-only; the idempotency key is unique per file.
create table ops (
  file_id         text not null references files(id) on delete cascade,
  seq             bigint not null,
  idempotency_key text not null,
  actor           text not null,
  command         jsonb not null,
  schema_version  integer not null check (schema_version >= 1),
  created_at      timestamptz not null default now(),
  primary key (file_id, seq),
  unique (file_id, idempotency_key)
);

create function ops_append_only() returns trigger as $$
begin
  raise exception 'ops are append-only';
end;
$$ language plpgsql;

create trigger ops_no_update before update on ops
  for each row execute function ops_append_only();

-- Periodic snapshots of the resolved file: fast load, and the fallback when a replay is older than the kept log.
-- The newest snapshot of a file is the one with the highest `seq`.
create table snapshots (
  file_id        text not null references files(id) on delete cascade,
  seq            bigint not null,
  state          jsonb not null,
  schema_version integer not null check (schema_version >= 1),
  created_at     timestamptz not null default now(),
  primary key (file_id, seq)
);
