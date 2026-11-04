// Re-embed migrations whenever one is added or changed: `sqlx::migrate!` reads
// them at compile time, and Cargo does not watch that directory on its own.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
