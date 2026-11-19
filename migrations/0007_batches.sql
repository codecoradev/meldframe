-- E5.1 (ADR-0021): the headers of the numbered batches of a file (`activity_store::BatchStore`), kept beside the op
-- log and never in it. A batch is the written ops of one actor whose keys are `{key_prefix}:{n}`; its header holds only
-- what the log cannot tell: its number in the file, its note, how many commands it has, and the review state a person
-- gave it. `commands` and `written` are counts kept beside the log (when they disagree the log wins) and only rise.
--
-- `batch_files` is one counter row per file: `next_n` is the number the next header takes (per file, from 1, never
-- reused), `next_rev` orders every change of a header of the file, `headers` counts them (the cap of the file, and the
-- sum over the files of a workspace is the cap of the workspace) and `open_headers` counts those that are not
-- accepted (`unreviewedCount`, so no count(*) runs under the lock). The per-actor cap is an indexed count over at most
-- 2,000 rows of `batches`. The adapter reads and updates the counter row under
-- `pg_advisory_xact_lock` on the workspace's batches, so the numbers are given one after the other and the caps hold
-- when calls run at once. The checks here repeat the bounds of one row, so a bug in an adapter cannot store more.
--
-- A header is written after its ops, so its `files` row exists, and both tables cascade with the file: a file name
-- that is recreated does not inherit an old header, note or review. Nothing deletes a header otherwise (the same rule
-- as `ops`, which cascades from `files`; `calls` is the table that has no cascade). An update may change only what
-- the trigger lets it: see `batches_guard`.

create table batch_files (
  file_id      text primary key references files(id) on delete cascade,
  workspace_id text not null,
  next_n       integer not null default 1 check (next_n >= 1),
  next_rev     bigint  not null default 1 check (next_rev >= 1),
  headers      integer not null default 0 check (headers >= 0),
  open_headers integer not null default 0 check (open_headers between 0 and headers)
);

create index batch_files_workspace on batch_files (workspace_id);

create table batches (
  file_id        text not null references files(id) on delete cascade,
  workspace_id   text not null,
  n              integer not null check (n >= 1),
  actor          text not null check (octet_length(actor) between 1 and 128),
  key_prefix     text not null check (char_length(key_prefix) between 1 and 200),
  tool           text not null check (octet_length(tool) between 1 and 64),
  commands       integer not null check (commands between 1 and 100000),
  written        integer not null check (written between 0 and 100000),
  atomic         boolean not null,
  note           text check (note is null or char_length(note) <= 500),
  review         text not null default 'unreviewed' check (review in ('unreviewed', 'accepted', 'needs_review')),
  reviewed_by    text check (reviewed_by is null or octet_length(reviewed_by) between 1 and 128),
  reviewed_at_ms bigint,
  rev            bigint not null check (rev >= 1),
  first_seq      bigint not null,
  last_seq       bigint not null,
  at_ms          bigint not null,
  created_at     timestamptz not null default now(),
  primary key (file_id, n),
  unique (file_id, actor, key_prefix),
  check (written <= commands),
  check (first_seq <= last_seq),
  check ((reviewed_by is null) = (reviewed_at_ms is null)),
  check (review = 'unreviewed' or reviewed_by is not null)
);

-- The ADR names it. The unique (file_id, actor, key_prefix) above also serves a prefix scan on (file_id, actor), so this
-- index is not needed by the per-actor count; it is kept as the ADR has it, a small one (the rows of an actor are at
-- most 2,000).
create index batches_actor on batches (file_id, actor);
-- The count of headers that are not accepted (`unreviewedCount`): every state but 'accepted'.
create index batches_unreviewed on batches (file_id) where review <> 'accepted';

-- An update may change `written` and `commands` (only upward), `last_seq` (only upward), `note` (only from null),
-- `rev` (only upward) and the three review columns (together, with who and when); anything else is refused, and a
-- change of any kind takes a higher `rev`.
create function batches_guard() returns trigger as $$
begin
  if new.file_id is distinct from old.file_id or new.workspace_id is distinct from old.workspace_id
     or new.n is distinct from old.n or new.actor is distinct from old.actor
     or new.key_prefix is distinct from old.key_prefix or new.tool is distinct from old.tool
     or new.atomic is distinct from old.atomic or new.first_seq is distinct from old.first_seq
     or new.at_ms is distinct from old.at_ms or new.created_at is distinct from old.created_at then
    raise exception 'a batch header keeps its identity, tool, first seq and time';
  end if;
  if new.commands < old.commands or new.written < old.written or new.last_seq < old.last_seq
     or new.rev < old.rev then
    raise exception 'a batch header only rises';
  end if;
  if old.note is not null and new.note is distinct from old.note then
    raise exception 'a batch note is set once';
  end if;
  if (new.review, new.reviewed_by, new.reviewed_at_ms) is distinct from (old.review, old.reviewed_by, old.reviewed_at_ms)
     and (new.reviewed_by is null or new.reviewed_at_ms is null) then
    raise exception 'a review is set together with who set it and when';
  end if;
  if new.rev <= old.rev and (new.commands, new.written, new.last_seq, new.note, new.review, new.reviewed_by,
       new.reviewed_at_ms) is distinct from (old.commands, old.written, old.last_seq, old.note, old.review,
       old.reviewed_by, old.reviewed_at_ms) then
    raise exception 'a change of a batch header takes the next rev';
  end if;
  return new;
end;
$$ language plpgsql;

create trigger batches_guard before update on batches
  for each row execute function batches_guard();

-- `next_n` and `next_rev` only rise (`headers` and `open_headers` move both ways, within their checks).
create function batch_files_guard() returns trigger as $$
begin
  if new.file_id is distinct from old.file_id or new.workspace_id is distinct from old.workspace_id then
    raise exception 'a batch counter row keeps its file';
  end if;
  if new.next_n < old.next_n or new.next_rev < old.next_rev then
    raise exception 'a batch counter only rises';
  end if;
  return new;
end;
$$ language plpgsql;

create trigger batch_files_guard before update on batch_files
  for each row execute function batch_files_guard();
