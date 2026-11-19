-- E5.4: the critique list of a file (`design_review::ReviewStore`) kept in Postgres. One row per item, numbered from 1
-- in each file of each workspace (the number is the primary key with the workspace and the file, so a workspace never
-- reaches another's list). Items are never deleted, so the next number is the highest plus one; the adapter takes a
-- transaction lock on the file's list before it reads that number. `batches` holds the key prefixes of the batches
-- linked to the item, in the order they were linked. The list is data next to the file, not part of its op log.

create table design_review_items (
  workspace_id text not null,
  file         text not null,
  n            integer not null check (n > 0),
  body         text not null check (body <> ''),
  actor        text not null,
  addressed    boolean not null default false,
  batches      text[] not null default '{}',
  created_at   timestamptz not null default now(),
  primary key (workspace_id, file, n)
);
