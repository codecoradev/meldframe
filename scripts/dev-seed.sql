-- Development seed: one workspace, one file and one development token.
-- Run through scripts/dev-seed.sh, which refuses release builds and non-development databases and
-- passes the hash of a random per-install token as :token_hash (no token value lives in the repository).
-- Idempotent. Until the real migrations land (E0.3, E1.2) the rows live in the `dev_seed` schema; the
-- seed then moves to the real tables with the same ids, and the token stays random per install.
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

-- One token per install: only issued when none exists for the actor.
INSERT INTO dev_seed.dev_token (token_sha256, workspace_id, actor)
SELECT :'token_hash', 'ws_dev', 'agent:dev'
WHERE NOT EXISTS (SELECT 1 FROM dev_seed.dev_token WHERE actor = 'agent:dev');

-- 1 when the token of this run was issued, 0 when an earlier one is kept.
SELECT count(*) FROM dev_seed.dev_token WHERE token_sha256 = :'token_hash';
