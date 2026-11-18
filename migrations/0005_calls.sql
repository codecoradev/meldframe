-- E2.4: the call log (`meldframe_calls::CallEntry`) kept in Postgres, so it survives a restart and can be read per
-- workspace (the activity panel of E5.1). One row per call: names and codes only, never node names, texts or
-- commands. Rows are appended and never changed.

create table calls (
  id           bigserial primary key,
  at_ms        bigint not null,
  workspace_id text not null,
  actor        text not null,
  tool         text not null,
  file         text not null,
  risk         text,
  decision     text,
  outcome      text not null
);

create index calls_workspace on calls (workspace_id, id);
