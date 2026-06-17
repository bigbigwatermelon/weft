//! Restore round-trip: back the db up, stage restore while the app DB is live,
//! then apply the staged restore before the next `Db::open_default`.

use atlas_app_lib::backup::{config, recovery_key, BackupService};
use atlas_app_lib::commands::ensure_default_workspace_inner;
use atlas_app_lib::store::key::{format_for_pragma, SqlCipherKey};
use atlas_app_lib::store::Db;
use base64::Engine;
use sea_orm::ConnectionTrait;
use sea_orm_migration::MigratorTrait;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn iso_env_with(home: &Path, key: [u8; 48]) {
    std::env::set_var("ATLAS_HOME", home);
    set_test_key(key);
}

fn set_test_key(key: [u8; 48]) {
    let b64 = base64::engine::general_purpose::STANDARD.encode(key);
    std::env::set_var("ATLAS_TEST_DB_KEY_B64", &b64);
}

fn current_schema_version() -> usize {
    atlas_app_lib::store::migration::Migrator::migrations().len()
}

fn make_bare(parent: &Path) -> String {
    let bare = parent.join("remote.git");
    let s = atlas_app_lib::git::command()
        .arg("init")
        .arg("--bare")
        .arg("--initial-branch=main")
        .arg(&bare)
        .status()
        .unwrap();
    assert!(s.success());
    format!("file://{}", bare.to_string_lossy())
}

fn sh(dir: &Path, args: &[&str]) {
    let mut cmd = atlas_app_lib::git::command();
    let st = cmd
        .args(&args[1..])
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(st.success(), "command failed: {args:?}");
}

fn make_remote_without_meta(parent: &Path) -> String {
    let url = make_bare(parent);
    let work = parent.join("invalid-work");
    std::fs::create_dir_all(&work).unwrap();
    sh(&work, &["git", "init", "--initial-branch=main"]);
    std::fs::write(work.join("README.md"), "not an Atlas backup").unwrap();
    sh(&work, &["git", "add", "README.md"]);
    sh(
        &work,
        &[
            "git",
            "-c",
            "user.email=atlas@local",
            "-c",
            "user.name=Atlas",
            "commit",
            "-m",
            "invalid backup",
        ],
    );
    sh(&work, &["git", "remote", "add", "origin", &url]);
    sh(&work, &["git", "push", "origin", "HEAD:refs/heads/main"]);
    url
}

async fn insert_workspace(db: &Db, name: &str) {
    db.0.execute_unprepared(&format!(
        "INSERT INTO workspace (id, name, slug, created_at) \
             VALUES (1, '{name}', '{name}', '1234567890')"
    ))
    .await
    .unwrap();
}

async fn workspace_name(db: &Db, id: i32) -> String {
    let row =
        db.0.query_one(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            format!("SELECT name FROM workspace WHERE id = {id}"),
        ))
        .await
        .unwrap()
        .expect("row exists");
    row.try_get("", "name").unwrap()
}

async fn workspace_count(db: &Db) -> i64 {
    let row =
        db.0.query_one(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM workspace".to_owned(),
        ))
        .await
        .unwrap()
        .expect("row exists");
    row.try_get("", "n").unwrap()
}

async fn thread_count(db: &Db) -> i64 {
    let row =
        db.0.query_one(sea_orm::Statement::from_string(
            sea_orm::DbBackend::Sqlite,
            "SELECT COUNT(*) AS n FROM thread".to_owned(),
        ))
        .await
        .unwrap()
        .expect("row exists");
    row.try_get("", "n").unwrap()
}

async fn insert_real_task(db: &Db) {
    let default_id = ensure_default_workspace_inner(db).await.unwrap();
    assert_eq!(default_id, 1);
    db.0.execute_unprepared(
        "INSERT INTO thread \
             (id, workspace_id, title, slug, kind, lead_tool, created_at) \
             VALUES (1, 1, 'real task', 'real-task', 'task', 'codex', '123')",
    )
    .await
    .unwrap();
}

async fn seed_current_default_task(home: &Path, key: [u8; 48]) {
    iso_env_with(home, key);
    let db = Db::open_default().await.unwrap();
    insert_real_task(&db).await;
    db.0.close().await.unwrap();
}

async fn assert_current_default_task(home: &Path, key: [u8; 48]) {
    iso_env_with(home, key);
    let db = Db::open_default().await.unwrap();
    assert_eq!(workspace_name(&db, 1).await, "Default");
    assert_eq!(thread_count(&db).await, 1);
}

fn failed_restore_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(home)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("pending-restore.failed."))
                .unwrap_or(false)
        })
        .collect();
    dirs.sort();
    dirs
}

async fn make_backup(
    root: &Path,
    source_home: &Path,
    key: [u8; 48],
    workspace: &str,
) -> (String, PathBuf) {
    std::fs::create_dir_all(source_home).unwrap();
    iso_env_with(source_home, key);
    let url = make_bare(root);

    {
        let db = Db::open_default().await.unwrap();
        config::save_prefs(
            &db,
            config::UpdatePrefs {
                enabled: true,
                remote_url: url.clone(),
                auto_backup_enabled: false,
                backup_on_exit: false,
            },
        )
        .await
        .unwrap();
        insert_workspace(&db, workspace).await;
        let svc = BackupService::new(db.clone(), source_home.to_path_buf());
        let r = svc.run_now().await.unwrap();
        assert!(matches!(
            r,
            atlas_app_lib::backup::RunOutcome::Success { .. }
        ));
    }

    let rk_path = root.join(format!("{workspace}-rk.json"));
    recovery_key::export_to(&rk_path).unwrap();
    (url, rk_path)
}

async fn make_backup_with_meta_override(
    root: &Path,
    key: [u8; 48],
    workspace: &str,
    meta: &str,
) -> (String, PathBuf) {
    let source_home = root.join(format!("{workspace}-home"));
    let (url, rk_path) = make_backup(root, &source_home, key, workspace).await;
    let work = root.join(format!("{workspace}-meta-work"));
    sh(
        root,
        &[
            "git",
            "clone",
            "--branch",
            "main",
            &url,
            work.to_str().unwrap(),
        ],
    );
    std::fs::write(work.join(".atlas-backup-meta.json"), meta).unwrap();
    sh(&work, &["git", "add", ".atlas-backup-meta.json"]);
    sh(
        &work,
        &[
            "git",
            "-c",
            "user.email=atlas@local",
            "-c",
            "user.name=Atlas",
            "commit",
            "-m",
            "corrupt backup meta",
        ],
    );
    sh(&work, &["git", "push", "origin", "HEAD:refs/heads/main"]);
    (url, rk_path)
}

async fn make_backup_with_extra_snapshot_migration(
    root: &Path,
    key: [u8; 48],
    workspace: &str,
) -> (String, PathBuf) {
    let source_home = root.join(format!("{workspace}-home"));
    let (url, rk_path) = make_backup(root, &source_home, key, workspace).await;
    let work = root.join(format!("{workspace}-extra-migration-work"));
    sh(
        root,
        &[
            "git",
            "clone",
            "--branch",
            "main",
            &url,
            work.to_str().unwrap(),
        ],
    );
    insert_snapshot_migration(&work.join("atlas.db"), key, "m9999_future").await;
    sh(&work, &["git", "add", "atlas.db"]);
    sh(
        &work,
        &[
            "git",
            "-c",
            "user.email=atlas@local",
            "-c",
            "user.name=Atlas",
            "commit",
            "-m",
            "add future migration",
        ],
    );
    sh(&work, &["git", "push", "origin", "HEAD:refs/heads/main"]);
    (url, rk_path)
}

async fn make_backup_with_duplicate_snapshot_migration(
    root: &Path,
    key: [u8; 48],
    workspace: &str,
) -> (String, PathBuf, String) {
    let source_home = root.join(format!("{workspace}-home"));
    let (url, rk_path) = make_backup(root, &source_home, key, workspace).await;
    let work = root.join(format!("{workspace}-duplicate-migration-work"));
    sh(
        root,
        &[
            "git",
            "clone",
            "--branch",
            "main",
            &url,
            work.to_str().unwrap(),
        ],
    );
    let duplicated = duplicate_snapshot_migration(&work.join("atlas.db"), key).await;
    sh(&work, &["git", "add", "atlas.db"]);
    sh(
        &work,
        &[
            "git",
            "-c",
            "user.email=atlas@local",
            "-c",
            "user.name=Atlas",
            "commit",
            "-m",
            "duplicate migration row",
        ],
    );
    sh(&work, &["git", "push", "origin", "HEAD:refs/heads/main"]);
    (url, rk_path, duplicated)
}

async fn make_non_atlas_backup(
    root: &Path,
    key: [u8; 48],
    schema_version: usize,
) -> (String, PathBuf) {
    let source_home = root.join("non-atlas-source-home");
    std::fs::create_dir_all(&source_home).unwrap();
    iso_env_with(&source_home, key);
    let rk_path = root.join("non-atlas-rk.json");
    recovery_key::export_to(&rk_path).unwrap();

    let url = make_bare(root);
    let work = root.join("non-atlas-work");
    std::fs::create_dir_all(&work).unwrap();
    sh(&work, &["git", "init", "--initial-branch=main"]);

    let fake_db = work.join("atlas.db");
    write_non_atlas_encrypted_db(&fake_db, key).await;
    let meta = serde_json::json!({
        "schema_version": schema_version,
        "snapshot_at": "0",
        "db_bytes": std::fs::metadata(&fake_db).unwrap().len(),
        "atlas_version": "0.1.0"
    });
    std::fs::write(
        work.join(".atlas-backup-meta.json"),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )
    .unwrap();

    sh(
        &work,
        &["git", "add", "atlas.db", ".atlas-backup-meta.json"],
    );
    sh(
        &work,
        &[
            "git",
            "-c",
            "user.email=atlas@local",
            "-c",
            "user.name=Atlas",
            "commit",
            "-m",
            "fake backup",
        ],
    );
    sh(&work, &["git", "remote", "add", "origin", &url]);
    sh(&work, &["git", "push", "origin", "HEAD:refs/heads/main"]);
    (url, rk_path)
}

async fn make_forged_core_table_backup(
    root: &Path,
    key: [u8; 48],
    schema_version: usize,
) -> (String, PathBuf) {
    let source_home = root.join("forged-source-home");
    std::fs::create_dir_all(&source_home).unwrap();
    iso_env_with(&source_home, key);
    let rk_path = root.join("forged-rk.json");
    recovery_key::export_to(&rk_path).unwrap();

    let url = make_bare(root);
    let work = root.join("forged-work");
    std::fs::create_dir_all(&work).unwrap();
    sh(&work, &["git", "init", "--initial-branch=main"]);

    let fake_db = work.join("atlas.db");
    write_forged_core_tables_db(&fake_db, key).await;
    let meta = serde_json::json!({
        "schema_version": schema_version,
        "snapshot_at": "0",
        "db_bytes": std::fs::metadata(&fake_db).unwrap().len(),
        "atlas_version": "0.1.0"
    });
    std::fs::write(
        work.join(".atlas-backup-meta.json"),
        serde_json::to_vec_pretty(&meta).unwrap(),
    )
    .unwrap();

    sh(
        &work,
        &["git", "add", "atlas.db", ".atlas-backup-meta.json"],
    );
    sh(
        &work,
        &[
            "git",
            "-c",
            "user.email=atlas@local",
            "-c",
            "user.name=Atlas",
            "commit",
            "-m",
            "forged backup",
        ],
    );
    sh(&work, &["git", "remote", "add", "origin", &url]);
    sh(&work, &["git", "push", "origin", "HEAD:refs/heads/main"]);
    (url, rk_path)
}

async fn write_non_atlas_encrypted_db(path: &Path, key: [u8; 48]) {
    let k = SqlCipherKey::from_bytes(&key).unwrap();
    let mut opt = sea_orm::ConnectOptions::new(format!("sqlite://{}?mode=rwc", path.display()));
    opt.sqlcipher_key(format_for_pragma(&k));
    let conn = sea_orm::Database::connect(opt).await.unwrap();
    conn.execute_unprepared("CREATE TABLE not_atlas (id INTEGER PRIMARY KEY);")
        .await
        .unwrap();
}

async fn insert_snapshot_migration(path: &Path, key: [u8; 48], version: &str) {
    let k = SqlCipherKey::from_bytes(&key).unwrap();
    let mut opt = sea_orm::ConnectOptions::new(format!("sqlite://{}?mode=rw", path.display()));
    opt.sqlcipher_key(format_for_pragma(&k));
    let conn = sea_orm::Database::connect(opt).await.unwrap();
    conn.execute_unprepared(&format!(
        "INSERT INTO seaql_migrations (version, applied_at) VALUES ('{version}', 0);"
    ))
    .await
    .unwrap();
}

async fn duplicate_snapshot_migration(path: &Path, key: [u8; 48]) -> String {
    let k = SqlCipherKey::from_bytes(&key).unwrap();
    let mut opt = sea_orm::ConnectOptions::new(format!("sqlite://{}?mode=rw", path.display()));
    opt.sqlcipher_key(format_for_pragma(&k));
    let conn = sea_orm::Database::connect(opt).await.unwrap();
    let row = conn
        .query_one(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "SELECT version FROM seaql_migrations ORDER BY version LIMIT 1".to_string(),
        ))
        .await
        .unwrap()
        .unwrap();
    let version: String = row.try_get("", "version").unwrap();
    let escaped = version.replace('\'', "''");
    conn.execute_unprepared(
        "CREATE TABLE seaql_migrations_dup (version TEXT NOT NULL, applied_at INTEGER NOT NULL);",
    )
    .await
    .unwrap();
    conn.execute_unprepared(
        "INSERT INTO seaql_migrations_dup (version, applied_at) \
         SELECT version, applied_at FROM seaql_migrations;",
    )
    .await
    .unwrap();
    conn.execute_unprepared(&format!(
        "INSERT INTO seaql_migrations_dup (version, applied_at) \
         SELECT version, applied_at FROM seaql_migrations WHERE version = '{escaped}' LIMIT 1;"
    ))
    .await
    .unwrap();
    conn.execute_unprepared("DROP TABLE seaql_migrations;")
        .await
        .unwrap();
    conn.execute_unprepared("ALTER TABLE seaql_migrations_dup RENAME TO seaql_migrations;")
        .await
        .unwrap();
    version
}

async fn write_forged_core_tables_db(path: &Path, key: [u8; 48]) {
    let k = SqlCipherKey::from_bytes(&key).unwrap();
    let mut opt = sea_orm::ConnectOptions::new(format!("sqlite://{}?mode=rwc", path.display()));
    opt.sqlcipher_key(format_for_pragma(&k));
    let conn = sea_orm::Database::connect(opt).await.unwrap();

    conn.execute_unprepared(
        "CREATE TABLE seaql_migrations (version TEXT PRIMARY KEY, applied_at INTEGER NOT NULL);",
    )
    .await
    .unwrap();
    for migration in atlas_app_lib::store::migration::Migrator::migrations() {
        let version = migration.name().replace('\'', "''");
        conn.execute_unprepared(&format!(
            "INSERT INTO seaql_migrations (version, applied_at) VALUES ('{version}', 0);"
        ))
        .await
        .unwrap();
    }

    for table in [
        "workspace",
        "backup_config",
        "thread",
        "session",
        "skill_source",
    ] {
        conn.execute_unprepared(&format!(
            "CREATE TABLE \"{}\" (id INTEGER PRIMARY KEY);",
            table.replace('"', "\"\"")
        ))
        .await
        .unwrap();
    }
}

fn copy_pending_snapshot_to_db(home: &Path) {
    let pending_snapshot = home.join("pending-restore").join("atlas.db");
    let db_path = home.join("atlas.db");
    let _ = std::fs::remove_file(home.join("atlas.db-wal"));
    let _ = std::fs::remove_file(home.join("atlas.db-shm"));
    let _ = std::fs::remove_file(home.join("atlas.db-journal"));
    std::fs::copy(pending_snapshot, db_path).unwrap();
}

async fn assert_restored_workspace(home: &Path, key: [u8; 48], name: &str) {
    iso_env_with(home, key);
    let db = Db::open_default().await.unwrap();
    assert_eq!(workspace_name(&db, 1).await, name);
}

#[cfg(unix)]
fn assert_pending_permissions(home: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let pending = home.join("pending-restore");
    let dir_mode = std::fs::metadata(&pending).unwrap().permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700);
    for name in ["manifest.json", "recovery-key.json", "atlas.db"] {
        let mode = std::fs::metadata(pending.join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{name}");
    }
}

#[cfg(not(unix))]
fn assert_pending_permissions(_home: &Path) {}

#[tokio::test]
async fn backup_then_restore_roundtrip() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0xEDu8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk_path) = make_backup(tmp.path(), &source_home, key, "restore-me").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);

    let svc = {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        BackupService::new(db, target_home.clone())
    };
    svc.restore_from(&url, &rk_path).await.unwrap();
    assert!(atlas_app_lib::backup::pending_restore_exists(&target_home));
    assert_pending_permissions(&target_home);
    assert!(!target_home.join("atlas.db").exists());

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));

    let db = Db::open_default().await.unwrap();
    assert_eq!(workspace_name(&db, 1).await, "restore-me");
}

#[tokio::test]
async fn apply_pending_restore_switches_from_shell_key_to_snapshot_key() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let old_key = [0xA1u8; 48];
    let new_key = [0xB2u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, new_key, "new-key-data").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, old_key);
    {
        let db = Db::open_default().await.unwrap();
        let _ = config::load(&db).await.unwrap();
        let svc = BackupService::new(db, target_home.clone());
        svc.restore_from(&url, &rk).await.unwrap();
    }

    set_test_key(old_key);
    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    assert_restored_workspace(&target_home, new_key, "new-key-data").await;
}

#[tokio::test]
async fn apply_pending_restore_recovers_after_db_swap_before_key_install() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let old_key = [0xA3u8; 48];
    let new_key = [0xB4u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, new_key, "swapped-before-key").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, old_key);
    {
        let db = Db::open_default().await.unwrap();
        let _ = config::load(&db).await.unwrap();
        let svc = BackupService::new(db.clone(), target_home.clone());
        svc.restore_from(&url, &rk).await.unwrap();
        drop(svc);
        db.0.close().await.unwrap();
    }
    copy_pending_snapshot_to_db(&target_home);

    set_test_key(old_key);
    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    assert_restored_workspace(&target_home, new_key, "swapped-before-key").await;
}

#[tokio::test]
async fn apply_pending_restore_cleans_pending_after_key_installed() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let old_key = [0xA5u8; 48];
    let new_key = [0xB6u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, new_key, "key-already-installed").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, old_key);
    {
        let db = Db::open_default().await.unwrap();
        let _ = config::load(&db).await.unwrap();
        let svc = BackupService::new(db.clone(), target_home.clone());
        svc.restore_from(&url, &rk).await.unwrap();
        drop(svc);
        db.0.close().await.unwrap();
    }
    copy_pending_snapshot_to_db(&target_home);

    set_test_key(new_key);
    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    assert_restored_workspace(&target_home, new_key, "key-already-installed").await;
}

#[tokio::test]
async fn restore_allows_empty_startup_db_and_does_not_replace_live_connection() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x44u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "restore-empty-target").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    {
        let db = Db::open_default().await.unwrap();
        let _ = config::load(&db).await.unwrap();
        let svc = BackupService::new(db.clone(), target_home.clone());
        svc.restore_from(&url, &rk).await.unwrap();
        assert!(atlas_app_lib::backup::pending_restore_exists(&target_home));
        assert_eq!(workspace_count(&db).await, 0);
    }

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    let db = Db::open_default().await.unwrap();
    assert_eq!(workspace_name(&db, 1).await, "restore-empty-target");
}

#[tokio::test]
async fn restore_allows_auto_default_workspace_shell() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x45u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "restore-default-target").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    {
        let db = Db::open_default().await.unwrap();
        let default_id = ensure_default_workspace_inner(&db).await.unwrap();
        assert_eq!(default_id, 1);
        let svc = BackupService::new(db.clone(), target_home.clone());
        svc.restore_from(&url, &rk).await.unwrap();
        assert!(atlas_app_lib::backup::pending_restore_exists(&target_home));
        assert_eq!(workspace_name(&db, 1).await, "Default");
    }

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    let db = Db::open_default().await.unwrap();
    assert_eq!(workspace_name(&db, 1).await, "restore-default-target");
}

#[tokio::test]
async fn restore_refuses_when_db_has_non_default_workspace() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x12u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "valid-source").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    let db = Db::open_default().await.unwrap();
    insert_workspace(&db, "keep-me").await;
    let svc = BackupService::new(db, target_home.clone());
    let err = svc.restore_from(&url, &rk).await.err().expect("must error");
    assert!(
        err.to_string().contains("contains existing Atlas data"),
        "got: {err:#}"
    );
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
}

#[tokio::test]
async fn restore_refuses_when_default_workspace_has_task() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x13u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "valid-source").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    let db = Db::open_default().await.unwrap();
    insert_real_task(&db).await;

    let svc = BackupService::new(db, target_home.clone());
    let err = svc.restore_from(&url, &rk).await.err().expect("must error");
    assert!(
        err.to_string().contains("contains existing Atlas data"),
        "got: {err:#}"
    );
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
}

#[tokio::test]
async fn invalid_recovery_key_does_not_delete_current_db_or_stage_restore() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("target-home");
    std::fs::create_dir_all(&home).unwrap();
    iso_env_with(&home, [0x31u8; 48]);

    let db = Db::open_default().await.unwrap();
    let _ = config::load(&db).await.unwrap();
    let svc = BackupService::new(db.clone(), home.clone());
    let bad_key = tmp.path().join("bad-rk.json");
    std::fs::write(
        &bad_key,
        br#"{"version":1,"service":"atlas","account":"db-key-v1","key_b64":"AA==","exported_at":"0","note":""}"#,
    )
    .unwrap();

    let err = svc
        .restore_from("file:///nonexistent", &bad_key)
        .await
        .err()
        .expect("must reject invalid key");
    assert!(err.to_string().contains("sqlcipher key"));
    assert!(home.join("atlas.db").exists());
    assert!(!atlas_app_lib::backup::pending_restore_exists(&home));
    assert_eq!(workspace_count(&db).await, 0);
}

#[tokio::test]
async fn invalid_remote_or_meta_does_not_delete_current_db_or_stage_restore() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("target-home");
    std::fs::create_dir_all(&home).unwrap();
    iso_env_with(&home, [0x32u8; 48]);

    let db = Db::open_default().await.unwrap();
    let _ = config::load(&db).await.unwrap();
    let rk = tmp.path().join("rk.json");
    recovery_key::export_to(&rk).unwrap();
    let svc = BackupService::new(db.clone(), home.clone());

    let err = svc
        .restore_from("file:///nonexistent", &rk)
        .await
        .err()
        .expect("must reject invalid remote");
    assert!(err.to_string().contains("git clone failed"));
    assert!(home.join("atlas.db").exists());
    assert!(!atlas_app_lib::backup::pending_restore_exists(&home));

    let bad_meta_root = tmp.path().join("bad-meta");
    std::fs::create_dir_all(&bad_meta_root).unwrap();
    let bad_meta_url = make_remote_without_meta(&bad_meta_root);
    let err = svc
        .restore_from(&bad_meta_url, &rk)
        .await
        .err()
        .expect("must reject invalid backup meta");
    assert!(err.to_string().contains("read backup meta"));
    assert!(home.join("atlas.db").exists());
    assert!(!atlas_app_lib::backup::pending_restore_exists(&home));
    assert_eq!(workspace_count(&db).await, 0);
}

#[tokio::test]
async fn malformed_meta_with_snapshot_does_not_stage_restore() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x33u8; 48];

    let missing_schema_root = tmp.path().join("missing-schema");
    std::fs::create_dir_all(&missing_schema_root).unwrap();
    let (missing_schema_url, missing_schema_rk) =
        make_backup_with_meta_override(&missing_schema_root, key, "missing-schema", "{}").await;

    let non_numeric_root = tmp.path().join("non-numeric-schema");
    std::fs::create_dir_all(&non_numeric_root).unwrap();
    let (non_numeric_url, non_numeric_rk) = make_backup_with_meta_override(
        &non_numeric_root,
        key,
        "non-numeric-schema",
        r#"{"schema_version":"1"}"#,
    )
    .await;

    let home = tmp.path().join("target-home");
    std::fs::create_dir_all(&home).unwrap();
    iso_env_with(&home, key);
    let db = Db::open_default().await.unwrap();
    let _ = config::load(&db).await.unwrap();
    let svc = BackupService::new(db.clone(), home.clone());

    for (url, rk) in [
        (missing_schema_url, missing_schema_rk),
        (non_numeric_url, non_numeric_rk),
    ] {
        let err = svc
            .restore_from(&url, &rk)
            .await
            .err()
            .expect("must reject malformed schema_version");
        assert!(
            err.to_string().contains("missing numeric schema_version"),
            "got: {err:#}"
        );
        assert!(home.join("atlas.db").exists());
        assert!(!atlas_app_lib::backup::pending_restore_exists(&home));
        assert_eq!(workspace_count(&db).await, 0);
    }
}

#[tokio::test]
async fn older_schema_backup_does_not_stage_restore() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x36u8; 48];
    let older_meta = format!(r#"{{"schema_version":{}}}"#, current_schema_version() - 1);
    let (url, rk) =
        make_backup_with_meta_override(tmp.path(), key, "older-schema", &older_meta).await;

    let home = tmp.path().join("target-home");
    std::fs::create_dir_all(&home).unwrap();
    iso_env_with(&home, key);
    let db = Db::open_default().await.unwrap();
    let _ = config::load(&db).await.unwrap();
    let svc = BackupService::new(db.clone(), home.clone());

    let err = svc
        .restore_from(&url, &rk)
        .await
        .err()
        .expect("must reject older schema backup");
    assert!(
        err.to_string()
            .contains("older backup requires a matching Atlas version"),
        "got: {err:#}"
    );
    assert!(home.join("atlas.db").exists());
    assert!(!atlas_app_lib::backup::pending_restore_exists(&home));
    assert_eq!(workspace_count(&db).await, 0);
}

#[tokio::test]
async fn malicious_pending_manifest_filename_is_quarantined() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x37u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "manifest-path").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    let svc = {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        BackupService::new(db, target_home.clone())
    };
    svc.restore_from(&url, &rk).await.unwrap();
    seed_current_default_task(&target_home, key).await;

    let manifest_path = target_home.join("pending-restore").join("manifest.json");
    let manifest = serde_json::json!({
        "version": 1,
        "schema_version": current_schema_version(),
        "staged_at": "0",
        "snapshot": "../atlas.db",
        "recovery_key": "recovery-key.json"
    });
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    let failed = failed_restore_dirs(&target_home);
    assert_eq!(failed.len(), 1);
    let note = std::fs::read_to_string(failed[0].join("restore-error.txt")).unwrap();
    assert!(note.contains("snapshot must be atlas.db"), "got: {note}");
    assert_current_default_task(&target_home, key).await;
}

#[tokio::test]
async fn malicious_pending_recovery_key_filename_is_quarantined() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x39u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "manifest-key-path").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    let svc = {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        BackupService::new(db, target_home.clone())
    };
    svc.restore_from(&url, &rk).await.unwrap();
    seed_current_default_task(&target_home, key).await;

    let manifest_path = target_home.join("pending-restore").join("manifest.json");
    let manifest = serde_json::json!({
        "version": 1,
        "schema_version": current_schema_version(),
        "staged_at": "0",
        "snapshot": "atlas.db",
        "recovery_key": "../recovery-key.json"
    });
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    let failed = failed_restore_dirs(&target_home);
    assert_eq!(failed.len(), 1);
    let note = std::fs::read_to_string(failed[0].join("restore-error.txt")).unwrap();
    assert!(
        note.contains("recovery_key must be recovery-key.json"),
        "got: {note}"
    );
    assert_current_default_task(&target_home, key).await;
}

#[tokio::test]
async fn older_pending_manifest_is_quarantined() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x38u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "pending-schema").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    let svc = {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        BackupService::new(db, target_home.clone())
    };
    svc.restore_from(&url, &rk).await.unwrap();
    seed_current_default_task(&target_home, key).await;

    let manifest_path = target_home.join("pending-restore").join("manifest.json");
    let manifest = serde_json::json!({
        "version": 1,
        "schema_version": current_schema_version() - 1,
        "staged_at": "0",
        "snapshot": "atlas.db",
        "recovery_key": "recovery-key.json"
    });
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    let failed = failed_restore_dirs(&target_home);
    assert_eq!(failed.len(), 1);
    let note = std::fs::read_to_string(failed[0].join("restore-error.txt")).unwrap();
    assert!(
        note.contains("older backup requires a matching Atlas version"),
        "got: {note}"
    );
    assert_current_default_task(&target_home, key).await;
}

#[tokio::test]
async fn missing_pending_recovery_key_is_quarantined() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x3Au8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "missing-key").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    let svc = {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        BackupService::new(db, target_home.clone())
    };
    svc.restore_from(&url, &rk).await.unwrap();
    seed_current_default_task(&target_home, key).await;
    std::fs::remove_file(
        target_home
            .join("pending-restore")
            .join("recovery-key.json"),
    )
    .unwrap();

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    let failed = failed_restore_dirs(&target_home);
    assert_eq!(failed.len(), 1);
    let note = std::fs::read_to_string(failed[0].join("restore-error.txt")).unwrap();
    assert!(note.contains("read recovery key"), "got: {note}");
    assert_current_default_task(&target_home, key).await;
}

#[tokio::test]
async fn corrupted_pending_snapshot_is_quarantined() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x3Bu8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "bad-snapshot").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    let svc = {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        BackupService::new(db, target_home.clone())
    };
    svc.restore_from(&url, &rk).await.unwrap();
    seed_current_default_task(&target_home, key).await;
    std::fs::write(
        target_home.join("pending-restore").join("atlas.db"),
        b"not an encrypted Atlas sqlite database",
    )
    .unwrap();

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    let failed = failed_restore_dirs(&target_home);
    assert_eq!(failed.len(), 1);
    let note = std::fs::read_to_string(failed[0].join("restore-error.txt")).unwrap();
    assert!(
        note.contains("pending restore validation failed before applying"),
        "got: {note}"
    );
    assert_current_default_task(&target_home, key).await;
}

#[tokio::test]
async fn non_atlas_encrypted_snapshot_does_not_stage_restore() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x34u8; 48];
    let (url, rk) = make_non_atlas_backup(tmp.path(), key, current_schema_version()).await;

    let home = tmp.path().join("target-home");
    std::fs::create_dir_all(&home).unwrap();
    iso_env_with(&home, key);
    let db = Db::open_default().await.unwrap();
    let _ = config::load(&db).await.unwrap();
    let svc = BackupService::new(db.clone(), home.clone());

    let err = svc
        .restore_from(&url, &rk)
        .await
        .err()
        .expect("must reject encrypted non-Atlas sqlite snapshot");
    assert!(
        err.to_string().contains("not an Atlas database"),
        "got: {err:#}"
    );
    assert!(home.join("atlas.db").exists());
    assert!(!atlas_app_lib::backup::pending_restore_exists(&home));
    assert_eq!(workspace_count(&db).await, 0);
}

#[tokio::test]
async fn snapshot_with_extra_migration_does_not_stage_restore() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x3Cu8; 48];
    let (url, rk) =
        make_backup_with_extra_snapshot_migration(tmp.path(), key, "future-migration").await;

    let home = tmp.path().join("target-home");
    std::fs::create_dir_all(&home).unwrap();
    iso_env_with(&home, key);
    let db = Db::open_default().await.unwrap();
    let _ = config::load(&db).await.unwrap();
    let svc = BackupService::new(db.clone(), home.clone());

    let err = svc
        .restore_from(&url, &rk)
        .await
        .err()
        .expect("must reject snapshot migration set mismatch");
    assert!(
        err.to_string().contains("migration set mismatch")
            && err.to_string().contains("m9999_future"),
        "got: {err:#}"
    );
    assert!(home.join("atlas.db").exists());
    assert!(!atlas_app_lib::backup::pending_restore_exists(&home));
    assert_eq!(workspace_count(&db).await, 0);
}

#[tokio::test]
async fn snapshot_with_duplicate_migration_row_does_not_stage_restore() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x3Du8; 48];
    let (url, rk, duplicated) =
        make_backup_with_duplicate_snapshot_migration(tmp.path(), key, "duplicate-migration").await;

    let home = tmp.path().join("target-home");
    std::fs::create_dir_all(&home).unwrap();
    iso_env_with(&home, key);
    let db = Db::open_default().await.unwrap();
    let _ = config::load(&db).await.unwrap();
    let svc = BackupService::new(db.clone(), home.clone());

    let err = svc
        .restore_from(&url, &rk)
        .await
        .err()
        .expect("must reject duplicate snapshot migration rows");
    assert!(
        err.to_string().contains("duplicate migration rows")
            && err.to_string().contains(&duplicated),
        "got: {err:#}"
    );
    assert!(home.join("atlas.db").exists());
    assert!(!atlas_app_lib::backup::pending_restore_exists(&home));
    assert_eq!(workspace_count(&db).await, 0);
}

#[tokio::test]
async fn forged_core_tables_without_columns_do_not_stage_restore() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x35u8; 48];
    let (url, rk) = make_forged_core_table_backup(tmp.path(), key, current_schema_version()).await;

    let home = tmp.path().join("target-home");
    std::fs::create_dir_all(&home).unwrap();
    iso_env_with(&home, key);
    let db = Db::open_default().await.unwrap();
    let _ = config::load(&db).await.unwrap();
    let svc = BackupService::new(db.clone(), home.clone());

    let err = svc
        .restore_from(&url, &rk)
        .await
        .err()
        .expect("must reject forged Atlas table names with incomplete columns");
    assert!(err.to_string().contains("missing column"), "got: {err:#}");
    assert!(home.join("atlas.db").exists());
    assert!(!atlas_app_lib::backup::pending_restore_exists(&home));
    assert_eq!(workspace_count(&db).await, 0);
}

#[tokio::test]
async fn apply_pending_restore_quarantines_pending_when_real_user_data_exists() {
    let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let key = [0x46u8; 48];
    let source_home = tmp.path().join("source-home");
    let (url, rk) = make_backup(tmp.path(), &source_home, key, "must-not-overwrite").await;

    let target_home = tmp.path().join("target-home");
    std::fs::create_dir_all(&target_home).unwrap();
    iso_env_with(&target_home, key);
    {
        let db = Db::open_default().await.unwrap();
        let default_id = ensure_default_workspace_inner(&db).await.unwrap();
        assert_eq!(default_id, 1);
        let svc = BackupService::new(db.clone(), target_home.clone());
        svc.restore_from(&url, &rk).await.unwrap();
        insert_real_task(&db).await;
        db.0.close().await.unwrap();
    }

    atlas_app_lib::backup::apply_pending_restore_before_open(&target_home)
        .await
        .unwrap();
    assert!(!atlas_app_lib::backup::pending_restore_exists(&target_home));
    let failed = failed_restore_dirs(&target_home);
    assert_eq!(failed.len(), 1);
    let note = std::fs::read_to_string(failed[0].join("restore-error.txt")).unwrap();
    assert!(
        note.starts_with("Atlas did not apply this pending restore.\n\n"),
        "got: {note}"
    );
    assert!(note.contains("contains existing Atlas data"), "got: {note}");

    let db = Db::open_default().await.unwrap();
    assert_eq!(workspace_name(&db, 1).await, "Default");
    assert_eq!(thread_count(&db).await, 1);
}
