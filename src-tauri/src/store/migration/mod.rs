use crate::store::entities::{
    app_setting, backup_config, direction, im_route, lead_message, session, skill_enable,
    skill_source, thread, workspace,
};
use sea_orm::{EntityTrait, Schema};
use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(M0001Init),
            Box::new(M0002RepoProfile),
            Box::new(M0003Plan),
            Box::new(M0004DirectionStatus),
            Box::new(M0005DirectionRepoReason),
            Box::new(M0006DropDirectionRepo),
            Box::new(M0007LeadMessage),
            Box::new(M0008DirectionMandate),
            Box::new(M0009DropThreadStatus),
            Box::new(M0010AppSetting),
            Box::new(M0011ThreadLeadTool),
            Box::new(M0012DropRepoDefaultTool),
            Box::new(M0013SkillSource),
            Box::new(M0014SkillEnable),
            Box::new(M0015ImRoute),
            Box::new(M0016BackupConfig),
            Box::new(M0017DropLegacyRepoModel),
        ]
    }
}

async fn drop_table_if_exists(manager: &SchemaManager<'_>, table: &str) -> Result<(), DbErr> {
    let r = manager
        .drop_table(Table::drop().table(Alias::new(table)).to_owned())
        .await;
    match r {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().to_lowercase().contains("no such table") => Ok(()),
        Err(e) => Err(e),
    }
}

async fn drop_column_if_exists(
    manager: &SchemaManager<'_>,
    table: &str,
    column: &str,
) -> Result<(), DbErr> {
    let r = manager
        .alter_table(
            Table::alter()
                .table(Alias::new(table))
                .drop_column(Alias::new(column))
                .to_owned(),
        )
        .await;
    match r {
        Ok(()) => Ok(()),
        Err(e)
            if {
                let s = e.to_string().to_lowercase();
                s.contains("no such column") || s.contains("no such table")
            } =>
        {
            Ok(())
        }
        Err(e) => Err(e),
    }
}

async fn clear_legacy_repo_session_native_ids(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let r = manager
        .get_connection()
        .execute_unprepared("UPDATE session SET native_session_id = NULL WHERE repo_id IS NOT NULL")
        .await;
    match r {
        Ok(_) => Ok(()),
        Err(e)
            if {
                let s = e.to_string().to_lowercase();
                s.contains("no such column") || s.contains("no such table")
            } =>
        {
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub struct M0001Init;

impl MigrationName for M0001Init {
    fn name(&self) -> &str {
        "m0001_init"
    }
}

impl M0001Init {
    /// Derive a CREATE TABLE statement from an entity, scoped to the backend.
    fn table<E: EntityTrait>(schema: &Schema, e: E) -> TableCreateStatement {
        let mut stmt = schema.create_table_from_entity(e);
        stmt.if_not_exists();
        stmt
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0001Init {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        manager
            .create_table(Self::table(&schema, workspace::Entity))
            .await?;
        manager
            .create_table(Self::table(&schema, thread::Entity))
            .await?;
        manager
            .create_table(Self::table(&schema, direction::Entity))
            .await?;
        manager
            .create_table(Self::table(&schema, session::Entity))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in ["session", "direction", "thread", "workspace"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).to_owned())
                .await?;
        }
        Ok(())
    }
}

/// Retained migration slot for databases that already recorded it. No-op for
/// the generic agent base.
pub struct M0002RepoProfile;

impl MigrationName for M0002RepoProfile {
    fn name(&self) -> &str {
        "m0002_repo_profile"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0002RepoProfile {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table_if_exists(manager, "repo_profile").await
    }
}

/// Retained migration slot for databases that already recorded it. No-op for
/// the generic agent base.
pub struct M0003Plan;

impl MigrationName for M0003Plan {
    fn name(&self) -> &str {
        "m0003_plan"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0003Plan {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table_if_exists(manager, "plan").await
    }
}

/// Adds the agent/human-driven status column to directions (§4.6).
pub struct M0004DirectionStatus;

impl MigrationName for M0004DirectionStatus {
    fn name(&self) -> &str {
        "m0004_direction_status"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0004DirectionStatus {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // M0001 reflects the current entity, so a FRESH db already has `status`;
        // this migration only matters for dbs created before the column existed.
        // sqlite has no ADD COLUMN IF NOT EXISTS, so tolerate the duplicate.
        let r = manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("direction"))
                    .add_column(
                        ColumnDef::new(Alias::new("status"))
                            .string()
                            .not_null()
                            .default("queued"),
                    )
                    .to_owned(),
            )
            .await;
        match r {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().to_lowercase().contains("duplicate column") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("direction"))
                    .drop_column(Alias::new("status"))
                    .to_owned(),
            )
            .await
    }
}

/// Retained migration slot for databases that already recorded it. No-op for
/// the generic agent base.
pub struct M0005DirectionRepoReason;

impl MigrationName for M0005DirectionRepoReason {
    fn name(&self) -> &str {
        "m0005_direction_repo_reason"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0005DirectionRepoReason {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for c in ["repo_id", "reason"] {
            drop_column_if_exists(manager, "direction", c).await?;
        }
        Ok(())
    }
}

/// Drops a legacy join table if present. Retained for migration history.
pub struct M0006DropDirectionRepo;

impl MigrationName for M0006DropDirectionRepo {
    fn name(&self) -> &str {
        "m0006_drop_direction_repo"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0006DropDirectionRepo {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_table_if_exists(manager, "direction_repo").await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible: the table is gone for good. No-op.
        Ok(())
    }
}

/// Adds the worker-mandate column to directions (plan+impl | impl-only). M0001
/// reflects the current entity, so a FRESH db already has it; this only matters
/// for dbs created before the column existed. sqlite has no ADD COLUMN IF NOT
/// EXISTS, so tolerate the duplicate.
pub struct M0008DirectionMandate;

impl MigrationName for M0008DirectionMandate {
    fn name(&self) -> &str {
        "m0008_direction_mandate"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0008DirectionMandate {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let r = manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("direction"))
                    .add_column(
                        ColumnDef::new(Alias::new("mandate"))
                            .string()
                            .not_null()
                            .default("plan+impl"),
                    )
                    .to_owned(),
            )
            .await;
        match r {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().to_lowercase().contains("duplicate column") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("direction"))
                    .drop_column(Alias::new("mandate"))
                    .to_owned(),
            )
            .await
    }
}

/// Drops the vestigial thread.status column: written once at insert ("active"),
/// never read or updated — the workspace board derives a thread's phase from
/// its directions. A FRESH db (M0001 reflects the entity) never has it; only
/// dbs created before the removal do, so tolerate the missing column.
pub struct M0009DropThreadStatus;

impl MigrationName for M0009DropThreadStatus {
    fn name(&self) -> &str {
        "m0009_drop_thread_status"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0009DropThreadStatus {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let r = manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("thread"))
                    .drop_column(Alias::new("status"))
                    .to_owned(),
            )
            .await;
        match r {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().to_lowercase().contains("no such column") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible: the dead column is gone for good. No-op.
        Ok(())
    }
}

/// Adds the chat timeline table for the lead console (and chat-mode workers).
pub struct M0007LeadMessage;

impl MigrationName for M0007LeadMessage {
    fn name(&self) -> &str {
        "m0007_lead_message"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0007LeadMessage {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        let mut stmt = schema.create_table_from_entity(lead_message::Entity);
        stmt.if_not_exists();
        manager.create_table(stmt).await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_lead_message_thread")
                    .table(Alias::new("lead_message"))
                    .col(Alias::new("thread_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("lead_message")).to_owned())
            .await?;
        Ok(())
    }
}

/// Adds the global key-value settings table (default-tool selection).
pub struct M0010AppSetting;

impl MigrationName for M0010AppSetting {
    fn name(&self) -> &str {
        "m0010_app_setting"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0010AppSetting {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        let mut stmt = schema.create_table_from_entity(app_setting::Entity);
        stmt.if_not_exists();
        manager.create_table(stmt).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("app_setting")).to_owned())
            .await?;
        Ok(())
    }
}

/// Adds thread.lead_tool (the CLI driving the thread's lead), stamped at
/// creation. Existing threads were always claude-led, so backfill "claude".
/// M0001 reflects the current entity, so a FRESH db already has the column;
/// sqlite has no ADD COLUMN IF NOT EXISTS, so tolerate the duplicate.
pub struct M0011ThreadLeadTool;

impl MigrationName for M0011ThreadLeadTool {
    fn name(&self) -> &str {
        "m0011_thread_lead_tool"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0011ThreadLeadTool {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let r = manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("thread"))
                    .add_column(
                        ColumnDef::new(Alias::new("lead_tool"))
                            .string()
                            .not_null()
                            .default("claude"),
                    )
                    .to_owned(),
            )
            .await;
        match r {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().to_lowercase().contains("duplicate column") => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("thread"))
                    .drop_column(Alias::new("lead_tool"))
                    .to_owned(),
            )
            .await
    }
}

/// Retained migration slot for databases that already recorded it. No-op for
/// the generic agent base.
pub struct M0012DropRepoDefaultTool;

impl MigrationName for M0012DropRepoDefaultTool {
    fn name(&self) -> &str {
        "m0012_drop_repo_default_tool"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0012DropRepoDefaultTool {
    async fn up(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible: the dead column is gone for good. No-op.
        Ok(())
    }
}

/// Adds the skill_source table (git-hosted skill sources).
pub struct M0013SkillSource;
impl MigrationName for M0013SkillSource {
    fn name(&self) -> &str {
        "m0013_skill_source"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for M0013SkillSource {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        let mut stmt = schema.create_table_from_entity(skill_source::Entity);
        stmt.if_not_exists();
        manager.create_table(stmt).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("skill_source")).to_owned())
            .await?;
        Ok(())
    }
}

/// Adds the skill_enable table (per-skill, per-scope enablement).
pub struct M0014SkillEnable;
impl MigrationName for M0014SkillEnable {
    fn name(&self) -> &str {
        "m0014_skill_enable"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for M0014SkillEnable {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        let mut stmt = schema.create_table_from_entity(skill_enable::Entity);
        stmt.if_not_exists();
        manager.create_table(stmt).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("skill_enable")).to_owned())
            .await?;
        Ok(())
    }
}

/// Adds the im_route table — task ↔ IM thread binding (spec §6, M2).
pub struct M0015ImRoute;
impl MigrationName for M0015ImRoute {
    fn name(&self) -> &str {
        "m0015_im_route"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for M0015ImRoute {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        let mut stmt = schema.create_table_from_entity(im_route::Entity);
        stmt.if_not_exists();
        manager.create_table(stmt).await?;
        // Composite unique: same Feishu thread can't bind to two tasks.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_im_route_thread_ref")
                    .table(Alias::new("im_route"))
                    .col(Alias::new("channel"))
                    .col(Alias::new("chat_id"))
                    .col(Alias::new("im_thread_ref"))
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("im_route")).to_owned())
            .await?;
        Ok(())
    }
}

/// Adds backup_config — singleton config for git-remote backup.
pub struct M0016BackupConfig;
impl MigrationName for M0016BackupConfig {
    fn name(&self) -> &str {
        "m0016_backup_config"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for M0016BackupConfig {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());
        let mut stmt = schema.create_table_from_entity(backup_config::Entity);
        stmt.if_not_exists();
        manager.create_table(stmt).await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("backup_config")).to_owned())
            .await?;
        Ok(())
    }
}

/// Removes the old repository/worktree delivery model from existing databases.
pub struct M0017DropLegacyRepoModel;
impl MigrationName for M0017DropLegacyRepoModel {
    fn name(&self) -> &str {
        "m0017_drop_legacy_repo_model"
    }
}
#[async_trait::async_trait]
impl MigrationTrait for M0017DropLegacyRepoModel {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in ["plan", "repo_profile", "worktree", "repo_ref", "direction_repo"] {
            drop_table_if_exists(manager, table).await?;
        }
        for column in ["branch", "repo_id", "reason"] {
            drop_column_if_exists(manager, "direction", column).await?;
        }
        clear_legacy_repo_session_native_ids(manager).await?;
        drop_column_if_exists(manager, "session", "repo_id").await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement};

    #[tokio::test]
    async fn m0017_clears_legacy_repo_session_native_ids_before_dropping_repo_id() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            r#"
            CREATE TABLE session (
                id INTEGER PRIMARY KEY,
                direction_id INTEGER NOT NULL,
                repo_id INTEGER,
                tool TEXT NOT NULL,
                cwd TEXT NOT NULL,
                native_session_id TEXT,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .await
        .unwrap();
        db.execute_unprepared(
            r#"
            INSERT INTO session
                (id, direction_id, repo_id, tool, cwd, native_session_id, status, created_at)
            VALUES
                (1, 10, 99, 'codex', '/legacy', 'legacy-native', 'idle', '2026-01-01'),
                (2, 10, NULL, 'codex', '/generic', 'generic-native', 'idle', '2026-01-01');
            "#,
        )
        .await
        .unwrap();

        let manager = SchemaManager::new(&db);
        M0017DropLegacyRepoModel.up(&manager).await.unwrap();

        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "SELECT id, native_session_id FROM session ORDER BY id".to_string(),
            ))
            .await
            .unwrap();
        let legacy_native: Option<String> = rows[0].try_get("", "native_session_id").unwrap();
        let generic_native: Option<String> = rows[1].try_get("", "native_session_id").unwrap();
        assert_eq!(legacy_native, None);
        assert_eq!(generic_native.as_deref(), Some("generic-native"));

        let columns = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                "PRAGMA table_info('session')".to_string(),
            ))
            .await
            .unwrap();
        let has_repo_id = columns.iter().any(|row| {
            let name: String = row.try_get("", "name").unwrap();
            name == "repo_id"
        });
        assert!(!has_repo_id);
    }
}
