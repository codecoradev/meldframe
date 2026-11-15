-- The Store seam scopes an idempotency key to an actor within a room (two agents may both send "k1"), so the
-- unique constraint of the op log follows: (file, actor, key) instead of (file, key).
alter table ops drop constraint ops_file_id_idempotency_key_key;
alter table ops add constraint ops_file_actor_key_key unique (file_id, actor, idempotency_key);
