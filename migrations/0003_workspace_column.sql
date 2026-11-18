-- E2.3: a file belongs to one workspace, in a column and not only in its key; an op knows its size.
--
-- Until now the file id was the room key `<byte length of the workspace>:<workspace><file>` (see `room_key` in
-- meldframe-store-pg). The two parts are now columns too, so the rows of a workspace can be counted and scoped by
-- a plain `where workspace_id = ...`. The id stays the key and the only unique one; ops and snapshots follow their
-- file. `ops.bytes` is the size of the command as the gate counts it (compact JSON), so a restarted server starts
-- from the same figure it had.
--
-- The migration takes a brief lock on `files` and `ops`; rows written by an older server during the deploy would
-- lack the new columns, so stop writers while it runs.

alter table files add column workspace_id text;
alter table files add column name text;

-- The key form is `0` or a number without leading zeros, a colon, then at least that many bytes. Anything else
-- (never written by the server) is kept whole as the name in a workspace the server cannot name: its names have
-- no control characters.
create function pg_temp.split_key(id text, out ws text, out nm text) as $$
declare
  n int;
  rest bytea;
begin
  if id ~ '^(0|[1-9][0-9]{0,8}):' then
    n := split_part(id, ':', 1)::int;
    rest := convert_to(substr(id, strpos(id, ':') + 1), 'UTF8');
    if n <= octet_length(rest) then
      ws := convert_from(substring(rest from 1 for n), 'UTF8');
      nm := convert_from(substring(rest from n + 1), 'UTF8');
      return;
    end if;
  end if;
  ws := E'\x01raw';
  nm := id;
exception when others then
  ws := E'\x01raw';
  nm := id;
end;
$$ language plpgsql;

update files set workspace_id = k.ws, name = k.nm from (select id, (pg_temp.split_key(id)).* from files) k where files.id = k.id;

alter table files alter column workspace_id set not null;
alter table files alter column name set not null;
create index files_workspace on files (workspace_id);

-- The size of each op: new rows carry the compact size from the server; old rows get the size of their stored
-- text, which is a little larger (jsonb prints a space after `:` and `,`). `ops` is append-only, so the
-- backfill switches the guard off for its own statement and back on.
alter table ops add column bytes integer;
alter table ops disable trigger ops_no_update;
update ops set bytes = octet_length(command::text);
alter table ops enable trigger ops_no_update;
alter table ops alter column bytes set not null;
