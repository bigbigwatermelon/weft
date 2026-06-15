//! Git-remote backup of the local SQLCipher database.
//!
//! - `config`: singleton backup_config repo
//! - `snapshot`: writes `atlas.db` + meta json to a staging dir
//! - `git_remote`: shells out to the system `git` CLI
//! - `recovery_key`: Recovery Key file format
//! - `scheduler`: hourly tick + on-exit hook
//!
//! Design: `DESIGN-2026-06-12-local-db-backup.md`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::store::key::SqlCipherKey;
use crate::store::Db;

pub mod config;
pub mod git_remote;
pub mod recovery_key;
pub mod scheduler;
pub mod snapshot;

const PENDING_RESTORE_DIR: &str = "pending-restore";
const PENDING_RESTORE_TMP_DIR: &str = "pending-restore.tmp";
const PENDING_MANIFEST: &str = "manifest.json";
const PENDING_RECOVERY_KEY: &str = "recovery-key.json";
const RESTORE_DB_TMP: &str = "atlas.db.restore-new";
const PENDING_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct PendingRestoreManifest {
    version: u32,
    schema_version: usize,
    staged_at: String,
    snapshot: String,
    recovery_key: String,
}

/// App-level backup handle. Held in Tauri state; scheduler and commands both
/// share the same instance so they cannot race.
#[derive(Clone)]
pub struct BackupService {
    db: Db,
    home: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Debug)]
pub enum RunOutcome {
    Disabled,
    Success { commit_sha: String, bytes: i64 },
}

impl BackupService {
    pub fn new(db: Db, home: PathBuf) -> Self {
        Self {
            db,
            home,
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    /// Trigger one backup. Failures are recorded into `backup_config.last_error`
    /// and surfaced as `Err`; we never panic. Serialized by `lock` so a
    /// scheduler tick can't collide with a manual `run_now`.
    pub async fn run_now(&self) -> Result<RunOutcome> {
        let _guard = self.lock.lock().await;
        let cfg = config::load(&self.db).await?;
        if !cfg.enabled || cfg.remote_url.is_empty() {
            return Ok(RunOutcome::Disabled);
        }

        let result = self.do_backup(&cfg.remote_url).await;
        match &result {
            Ok((sha, bytes)) => {
                config::record_success(
                    &self.db,
                    config::BackupOutcome {
                        commit_sha: sha.clone(),
                        bytes: *bytes,
                    },
                )
                .await?;
            }
            Err(e) => {
                let msg = format!("{e:#}");
                let _ = config::record_failure(&self.db, &msg).await;
            }
        }
        let (sha, bytes) = result?;
        Ok(RunOutcome::Success {
            commit_sha: sha,
            bytes,
        })
    }

    async fn do_backup(&self, remote_url: &str) -> Result<(String, i64)> {
        git_remote::ensure_git_available()?;
        let staging = self.staging_dir(remote_url);
        git_remote::ensure_clone(&staging, remote_url)?;

        let bytes = snapshot::write_snapshot(&self.db, &staging).await?;

        let msg = format!("snapshot at {}", unix_now());
        let report = git_remote::commit_and_push(&staging, &msg)?;
        // `bytes_pushed` is a rough wd-bytes sum; fall back to snapshot size
        // if for some reason it came back zero.
        Ok((report.commit_sha, report.bytes_pushed.max(bytes)))
    }

    /// Deterministic per-remote staging path under `<home>/backup/<sha1[..8]>`.
    /// Same URL → same dir, so repeat backups reuse the clone instead of
    /// re-cloning each tick.
    pub fn staging_dir(&self, remote_url: &str) -> PathBuf {
        use sha1::{Digest, Sha1};
        let mut h = Sha1::new();
        h.update(remote_url.as_bytes());
        let digest = hex::encode(h.finalize());
        self.home.join("backup").join(&digest[..8])
    }

    /// Pull a backup snapshot down from `remote_url`, validate it against this
    /// build's schema, and stage it under `<home>/pending-restore`.
    ///
    /// Safety: this does not replace the live database or mutate Keychain while
    /// the app-wide SQLCipher connection is open. The staged restore is applied
    /// on the next startup, before `Db::open_default` creates the shared
    /// connection.
    pub async fn restore_from(&self, remote_url: &str, recovery_key_path: &Path) -> Result<()> {
        let _guard = self.lock.lock().await;
        let db_path = self.home.join("atlas.db");
        self.ensure_restore_target_is_shell(&db_path).await?;

        let imported = recovery_key::read_from(recovery_key_path)?;
        let tmp = self.home.join("backup-restore-tmp");
        if tmp.exists() {
            std::fs::remove_dir_all(&tmp)?;
        }

        let result = async {
            git_remote::clone_to(&tmp, remote_url)?;

            let backup_version = read_backup_schema_version(&tmp)?;
            let current_version = current_schema_version();
            if backup_version > current_version {
                return Err(anyhow::anyhow!(
                    "backup schema {backup_version} is newer than this Atlas ({current_version}); upgrade Atlas first"
                ));
            }

            let snap = tmp.join(snapshot::SNAPSHOT_NAME);
            if !snap.exists() {
                return Err(anyhow::anyhow!(
                    "backup repo missing atlas.db: {}",
                    snap.display()
                ));
            }
            verify_snapshot_with_key(&snap, &imported).await?;
            self.stage_pending_restore(&snap, recovery_key_path, backup_version)?;
            Ok(())
        }
        .await;

        let _ = std::fs::remove_dir_all(&tmp);
        result
    }

    fn stage_pending_restore(
        &self,
        snapshot_path: &Path,
        recovery_key_path: &Path,
        schema_version: usize,
    ) -> Result<()> {
        let pending = pending_restore_dir(&self.home);
        let tmp = self.home.join(PENDING_RESTORE_TMP_DIR);
        if tmp.exists() {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        std::fs::create_dir_all(&tmp)?;

        let staged_snapshot = tmp.join(snapshot::SNAPSHOT_NAME);
        std::fs::copy(snapshot_path, &staged_snapshot)?;
        let staged_key = tmp.join(PENDING_RECOVERY_KEY);
        std::fs::copy(recovery_key_path, &staged_key)?;

        let manifest = PendingRestoreManifest {
            version: PENDING_VERSION,
            schema_version,
            staged_at: unix_now(),
            snapshot: snapshot::SNAPSHOT_NAME.into(),
            recovery_key: PENDING_RECOVERY_KEY.into(),
        };
        std::fs::write(
            tmp.join(PENDING_MANIFEST),
            serde_json::to_vec_pretty(&manifest)?,
        )?;

        if pending.exists() {
            std::fs::remove_dir_all(&pending)?;
        }
        std::fs::rename(&tmp, &pending)?;
        Ok(())
    }

    async fn ensure_restore_target_is_shell(&self, db_path: &Path) -> Result<()> {
        if !db_path.exists() {
            return Ok(());
        }

        ensure_connection_is_shell(&self.db.0, db_path).await
    }
}

/// Apply a staged restore before the shared app database connection is opened.
/// If existing `atlas.db` contains real user data, this returns an error and
/// leaves the pending restore intact for user intervention.
pub async fn apply_pending_restore_before_open(home: &Path) -> Result<()> {
    let pending = pending_restore_dir(home);
    if !pending.exists() {
        return Ok(());
    }

    let manifest = read_pending_manifest(&pending)?;
    let snapshot_path = pending.join(&manifest.snapshot);
    let recovery_key_path = pending.join(&manifest.recovery_key);
    if !snapshot_path.exists() {
        return Err(anyhow::anyhow!(
            "pending restore missing atlas.db: {}",
            snapshot_path.display()
        ));
    }

    let imported = recovery_key::read_from(&recovery_key_path)?;
    verify_snapshot_with_key(&snapshot_path, &imported).await?;

    let db_path = home.join(snapshot::SNAPSHOT_NAME);
    if db_path.exists() {
        let current_key = crate::store::key::get_existing()?;
        let conn = open_sqlcipher_existing(&db_path, &current_key).await?;
        ensure_connection_is_shell(&conn, &db_path).await?;
        drop(conn);
    }

    recovery_key::install_key(&imported)?;
    replace_db_with_snapshot(home, &snapshot_path)?;
    std::fs::remove_dir_all(&pending)?;
    Ok(())
}

pub fn pending_restore_exists(home: &Path) -> bool {
    pending_restore_dir(home).exists()
}

fn pending_restore_dir(home: &Path) -> PathBuf {
    home.join(PENDING_RESTORE_DIR)
}

fn read_pending_manifest(pending: &Path) -> Result<PendingRestoreManifest> {
    let path = pending.join(PENDING_MANIFEST);
    let bytes = std::fs::read(&path)
        .map_err(|e| anyhow::anyhow!("read pending restore manifest {}: {e}", path.display()))?;
    let manifest: PendingRestoreManifest = serde_json::from_slice(&bytes)?;
    if manifest.version != PENDING_VERSION {
        return Err(anyhow::anyhow!(
            "unsupported pending restore version: {}",
            manifest.version
        ));
    }
    let current = current_schema_version();
    if manifest.schema_version > current {
        return Err(anyhow::anyhow!(
            "backup schema {} is newer than this Atlas ({current}); upgrade Atlas first",
            manifest.schema_version
        ));
    }
    Ok(manifest)
}

fn read_backup_schema_version(clone_dir: &Path) -> Result<usize> {
    let meta_path = clone_dir.join(".atlas-backup-meta.json");
    let meta_bytes = std::fs::read(&meta_path)
        .map_err(|e| anyhow::anyhow!("read backup meta {}: {e}", meta_path.display()))?;
    let meta: serde_json::Value = serde_json::from_slice(&meta_bytes)?;
    Ok(meta["schema_version"].as_u64().unwrap_or(0) as usize)
}

fn current_schema_version() -> usize {
    use sea_orm_migration::MigratorTrait;
    crate::store::migration::Migrator::migrations().len()
}

async fn verify_snapshot_with_key(path: &Path, key: &SqlCipherKey) -> Result<()> {
    let conn = open_sqlcipher_existing(path, key).await?;
    use sea_orm::ConnectionTrait;
    conn.execute_unprepared("SELECT name FROM sqlite_master LIMIT 1;")
        .await?;
    Ok(())
}

async fn open_sqlcipher_existing(
    path: &Path,
    key: &SqlCipherKey,
) -> Result<sea_orm::DatabaseConnection> {
    let url = format!("sqlite://{}?mode=rw", path.to_string_lossy());
    let mut opt = sea_orm::ConnectOptions::new(url);
    opt.sqlcipher_key(crate::store::key::format_for_pragma(key));
    Ok(sea_orm::Database::connect(opt).await?)
}

async fn ensure_connection_is_shell(
    conn: &sea_orm::DatabaseConnection,
    db_path: &Path,
) -> Result<()> {
    let rows = user_data_row_count(conn).await?;
    if rows > 0 {
        return Err(anyhow::anyhow!(
            "refusing to restore: {} contains existing Atlas data",
            db_path.display()
        ));
    }
    Ok(())
}

async fn user_data_row_count(conn: &sea_orm::DatabaseConnection) -> Result<i64> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    const USER_TABLES: &[&str] = &[
        "app_setting",
        "repo_ref",
        "repo_profile",
        "thread",
        "direction",
        "worktree",
        "session",
        "plan",
        "lead_message",
        "skill_source",
        "skill_enable",
        "im_route",
    ];

    let mut total = 0_i64;
    for table in USER_TABLES {
        let row = conn
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("SELECT COUNT(*) AS n FROM {table}"),
            ))
            .await?
            .ok_or_else(|| anyhow::anyhow!("count {table}: no row"))?;
        let count: i64 = row.try_get("", "n")?;
        total += count;
    }
    total += non_shell_workspace_row_count(conn).await?;
    total += non_shell_backup_config_row_count(conn).await?;
    Ok(total)
}

async fn non_shell_workspace_row_count(conn: &sea_orm::DatabaseConnection) -> Result<i64> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    let rows = conn
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name, slug FROM workspace".to_string(),
        ))
        .await?;
    match rows.as_slice() {
        [] => Ok(0),
        [row] => {
            let name: String = row.try_get("", "name")?;
            let slug: String = row.try_get("", "slug")?;
            if name == "Default" && slug == "default" {
                Ok(0)
            } else {
                Ok(1)
            }
        }
        _ => Ok(rows.len() as i64),
    }
}

async fn non_shell_backup_config_row_count(conn: &sea_orm::DatabaseConnection) -> Result<i64> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    let row = conn
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM backup_config \
             WHERE NOT (id = 1 \
               AND enabled = 0 \
               AND remote_url = '' \
               AND auto_backup_enabled = 1 \
               AND interval_seconds = 3600 \
               AND backup_on_exit = 1 \
               AND last_backup_at IS NULL \
               AND last_backup_commit_sha IS NULL \
               AND last_backup_bytes IS NULL \
               AND last_error IS NULL)"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| anyhow::anyhow!("count backup_config: no row"))?;
    Ok(row.try_get("", "n")?)
}

fn replace_db_with_snapshot(home: &Path, snapshot_path: &Path) -> Result<()> {
    let db_path = home.join(snapshot::SNAPSHOT_NAME);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = home.join(RESTORE_DB_TMP);
    if tmp.exists() {
        std::fs::remove_file(&tmp)?;
    }
    std::fs::copy(snapshot_path, &tmp)?;

    remove_file_if_exists(&home.join("atlas.db-wal"))?;
    remove_file_if_exists(&home.join("atlas.db-shm"))?;
    remove_file_if_exists(&home.join("atlas.db-journal"))?;
    remove_file_if_exists(&db_path)?;
    std::fs::rename(&tmp, &db_path)?;
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::anyhow!("remove {}: {e}", path.display())),
    }
}

fn unix_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn iso_env(home: &std::path::Path) {
        std::env::set_var("ATLAS_HOME", home);
        let raw = [0xA1u8; 48];
        let b64 = base64::engine::general_purpose::STANDARD.encode(raw);
        std::env::set_var("ATLAS_TEST_DB_KEY_B64", &b64);
    }

    #[tokio::test]
    async fn run_now_returns_disabled_when_unconfigured() {
        let _g = crate::paths::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        iso_env(tmp.path());
        let db = Db::open_default().await.unwrap();
        let svc = BackupService::new(db, tmp.path().to_path_buf());
        let r = svc.run_now().await.unwrap();
        assert!(matches!(r, RunOutcome::Disabled));
    }

    #[tokio::test]
    async fn staging_dir_is_deterministic_per_url() {
        let _g = crate::paths::ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        iso_env(tmp.path());
        let db = Db::open_default().await.unwrap();
        let svc = BackupService::new(db, tmp.path().to_path_buf());
        let a = svc.staging_dir("git@host:r.git");
        let b = svc.staging_dir("git@host:r.git");
        let c = svc.staging_dir("git@host:other.git");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with(tmp.path().join("backup")));
    }
}
