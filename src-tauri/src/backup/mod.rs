//! Git-remote backup of the local SQLCipher database.
//!
//! - `config`: singleton backup_config repo
//! - `snapshot`: writes `weft.db` + meta json to a staging dir
//! - `git_remote`: shells out to the system `git` CLI
//! - `recovery_key`: Recovery Key file format
//! - `scheduler`: hourly tick + on-exit hook
//!
//! Design: `DESIGN-2026-06-12-local-db-backup.md`.

pub mod config;
pub mod git_remote;
pub mod recovery_key;
pub mod scheduler;
pub mod snapshot;
