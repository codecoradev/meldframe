-- E2.4: the call log (`meldframe_calls::CallEntry`) kept in Postgres, so it survives a restart and can be read per
-- workspace (the activity panel of E5.1). One row per call: names and codes only, never node names, texts or
-- commands. Rows are appended and never changed (a trigger refuses an update). Nothing deletes them yet: a retention
-- rule is a follow-up.

create table calls (
  id           bigserial primary key,
  at_ms        bigint not null,
  workspace_id text not null,
  actor        text not null,
  tool         text not null,
  file         text not null,
  risk         text check (risk in ('read', 'change', 'delete')),
  decision     text check (decision in ('allow', 'ask', 'deny')),
  outcome      text not null
);

create function calls_append_only() returns trigger as $$
begin
  raise exception 'calls are append-only';
end;
$$ language plpgsql;

create trigger calls_no_update before update on calls
  for each row execute function calls_append_only();

create index calls_workspace on calls (workspace_id, id);
