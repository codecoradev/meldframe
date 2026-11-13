-- Development seed: one workspace, one file and one development token.
-- Run through scripts/dev-seed.sh, which refuses release builds. Idempotent.
-- Until the real migrations land (E0.3, E1.2) the rows live in the `dev_seed` schema; the
-- seed then moves to the real tables and keeps the same ids and token.
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE SCHEMA IF NOT EXISTS dev_seed;

CREATE TABLE IF NOT EXISTS dev_seed.workspace (
  id   text PRIMARY KEY,
  name text NOT NULL
);
CREATE TABLE IF NOT EXISTS dev_seed.file (
  id           text PRIMARY KEY,
  workspace_id text NOT NULL REFERENCES dev_seed.workspace (id),
  name         text NOT NULL
);
CREATE TABLE IF NOT EXISTS dev_seed.dev_token (
  token_sha256 text PRIMARY KEY,
  workspace_id text NOT NULL REFERENCES dev_seed.workspace (id),
  actor        text NOT NULL
);

INSERT INTO dev_seed.workspace (id, name)
VALUES ('ws_dev', 'Development workspace')
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;

INSERT INTO dev_seed.file (id, workspace_id, name)
VALUES ('file_dev', 'ws_dev', 'Development file')
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;

-- Token value is public on purpose: mf_dev_token_do_not_use_in_production
INSERT INTO dev_seed.dev_token (token_sha256, workspace_id, actor)
VALUES (encode(digest('mf_dev_token_do_not_use_in_production', 'sha256'), 'hex'), 'ws_dev', 'agent:dev')
ON CONFLICT (token_sha256) DO NOTHING;
