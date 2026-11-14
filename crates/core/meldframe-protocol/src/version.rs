/// Version of the stored file and snapshot shape. Every stored file and snapshot carries it (column
/// `schema_version`, see `migrations/0001_init.sql`). Bump it when a change to [`crate::Node`] or
/// [`crate::Command`] needs a migration of stored data; purely additive optional fields do not.
pub const SCHEMA_VERSION: u32 = 1;
