/// Storage module.
///
/// Responsible for:
/// - Initialising the local SQLite database
/// - CRUD operations on transcript sessions
/// - Preparing the data model to be sync-ready (UUIDs, timestamps, version vectors)
pub mod db;
pub mod models;
pub mod repository;
