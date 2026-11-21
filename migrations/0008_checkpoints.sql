-- E5.3 (ADR-0023): the checkpoints of a file (`checkpoint_store::CheckpointStore`), rows kept beside the op log and
-- never in it. A checkpoint is a number in the file's log (`seq`: the file as it was after op `seq`), never a copy of
-- the file; what it holds is who it is about, who asked for it, why, and a label a person typed.
--
-- `checkpoint_files` is one counter row per file: `next_n` is the number the next row takes (per file, from 1, never
-- reused, even when a roll deletes the row that held a number), and the three counts are the rows of each share of
-- the file (`manual`, `beforeRevert` and `beforeRestore` together, `runStart`), so a sum over the files of a workspace
-- can be added later without a change of schema (decision 39 takes no workspace figure now). The adapter reads and
-- updates the counter row under `pg_advisory_xact_lock` on the workspace's checkpoints, with a key prefix of its own
-- (`checkpoints:`), so numbers are given one after the other and the shares hold when calls run at once.
--
-- The checks here repeat the bounds of one row, so a bug in an adapter cannot store more: the four kinds, the shape of
-- each (a label only on `manual`, a run start about an actor and by nobody, every other kind by an actor, `about` only
-- on a revert or a run start), the lengths of the label (128 characters) and of the actors and the key (bytes, as the
-- batch headers of 0007 do), and the shares of a file. A row never changes (an update trigger refuses every change, so
-- a label cannot be edited); a roll is the adapter's own delete of an automatic row, so there is no delete trigger,
-- because a file's rows must go with the file: both tables cascade, so a recreated file name inherits nothing.
--
-- Two unique indexes make a retry safe: a request with a key is one row per file, actor and key, and a run start is
-- one row per file, actor and `seq` (a restart, a retry of the sink and a repeated call make no second row).

create table checkpoint_files (
  file_id      text primary key references files(id) on delete cascade,
  workspace_id text not null,
  next_n       integer not null default 1 check (next_n >= 1),
  manual_rows  integer not null default 0 check (manual_rows between 0 and 50),
  before_rows  integer not null default 0 check (before_rows between 0 and 25),
  run_rows     integer not null default 0 check (run_rows between 0 and 25)
);

create index checkpoint_files_workspace on checkpoint_files (workspace_id);

create table checkpoints (
  file_id      text not null references files(id) on delete cascade,
  workspace_id text not null,
  n            integer not null check (n >= 1),
  seq          bigint  not null check (seq >= 0),
  kind         text    not null check (kind in ('runStart', 'beforeRevert', 'beforeRestore', 'manual')),
  label        text    check (label is null or (kind = 'manual' and char_length(label) between 1 and 128)),
  about        text    check (about is null or octet_length(about) between 1 and 128),
  by           text    check (by is null or octet_length(by) between 1 and 128),
  key          text    check (key is null or octet_length(key) between 1 and 100),
  at_ms        bigint  not null,
  created_at   timestamptz not null default now(),
  primary key (file_id, n),
  -- A run start is the server's: about an actor, by nobody, with no label and no key.
  check (kind <> 'runStart' or (about is not null and by is null and label is null and key is null)),
  -- Every other kind is a person's call, by an actor.
  check (kind = 'runStart' or by is not null),
  -- `about` only on a revert (the actor of the batch) or a run start.
  check (kind in ('runStart', 'beforeRevert') or about is null)
);

-- A request with a key is one row per file, actor and key.
create unique index checkpoints_key on checkpoints (file_id, by, key) where key is not null;
-- A run start is one row per file, actor and `seq`.
create unique index checkpoints_run_start on checkpoints (file_id, about, seq) where kind = 'runStart';

-- A row never changes: not its label, not anything.
create function checkpoints_immutable() returns trigger as $$
begin
  raise exception 'a checkpoint never changes';
end;
$$ language plpgsql;

create trigger checkpoints_immutable before update on checkpoints
  for each row execute function checkpoints_immutable();

-- `next_n` only rises, so a number is never given twice.
create function checkpoint_files_guard() returns trigger as $$
begin
  if new.file_id is distinct from old.file_id or new.workspace_id is distinct from old.workspace_id then
    raise exception 'a checkpoint counter row keeps its file';
  end if;
  if new.next_n < old.next_n then
    raise exception 'a checkpoint counter only rises';
  end if;
  return new;
end;
$$ language plpgsql;

create trigger checkpoint_files_guard before update on checkpoint_files
  for each row execute function checkpoint_files_guard();
