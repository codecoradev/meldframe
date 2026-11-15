//! `sqlx::migrate!` embeds `migrations/` at compile time; without this Cargo would not rebuild the crate
//! when a migration file is added or edited.

fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
