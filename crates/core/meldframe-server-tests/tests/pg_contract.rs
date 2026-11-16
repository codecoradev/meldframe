//! The Postgres adapter passes the same store contract as the in-memory one.

mod common;

meldframe_store_contract::store_contract!(common::fresh);
