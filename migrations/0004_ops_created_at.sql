-- E2.3: the per-minute quota of a restarted server counts the ops a workspace wrote since the start of the
-- current minute (`PgStore::ops_since`). Without an index on the time, that count reads every op of the
-- workspace; with this one it reads the ops of the minute per file.

create index ops_file_created_at on ops (file_id, created_at);
