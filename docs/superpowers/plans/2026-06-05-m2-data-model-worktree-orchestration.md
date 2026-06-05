# M2 Data Model + Worktree Orchestration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist weft's workspace model in SQLite and orchestrate stable, parallel-safe git worktrees, so the M1 single-session demo becomes a real multi-thread / multi-direction workspace.

**Architecture:** A `store` module (SeaORM entities + migrations + repository fns) holds the model. A `slug` module produces filesystem- and git-ref-safe identifiers. `git.rs` gains worktree listing / diff / robust removal. `materialize.rs` turns a direction's write-scope into worktrees under a persistent home (`~/.weft/worktrees/...`). `pty.rs` is refactored from single-session to a `HashMap<session_id, Active>` and `open_session` operates on an already-materialized worktree. New Tauri commands expose all of it.

**Tech Stack:** Rust, Tauri v2, SeaORM (async ORM over sqlx, SQLite), system `git`, the existing `portable-pty` core.

---

## Reference: spec

`docs/superpowers/specs/2026-06-05-m2-data-model-worktree-orchestration-design.md`. Read it before starting.

## File structure (created/modified)

```
src-tauri/
  Cargo.toml                         # MODIFY: add sea-orm, sea-orm-migration, dirs, futures
  src/lib.rs                         # MODIFY: manage DbState, register M2 commands
  src/paths.rs                       # CREATE: ~/.weft home, db path, worktree home
  src/slug.rs                        # CREATE: slugify + unique_slug
  src/store/mod.rs                   # CREATE: Db handle, connect+migrate
  src/store/entities/mod.rs          # CREATE: re-exports
  src/store/entities/{workspace,repo_ref,thread,direction,direction_repo,worktree,session}.rs  # CREATE
  src/store/migration/mod.rs         # CREATE: Migrator + one migration
  src/store/repo.rs                  # CREATE: repository fns (CRUD + cascade)
  src/git.rs                         # MODIFY: list_worktrees, repo_diff, remove already exists
  src/materialize.rs                 # CREATE: materialize_direction
  src/pty.rs                         # MODIFY: multi-session, open_session(direction_id, repo_id)
  src/commands.rs                    # CREATE: thin Tauri command wrappers over store/materialize
  tests/m2_worktree.rs               # CREATE: integration tests (3 acceptance scenarios)
```

## Shared type decisions (consistent across all tasks)

- **Primary keys:** `i32`, autoincrement. Foreign keys are `i32`.
- **Timestamps:** stored as RFC3339 `String` (`created_at: String`).
- **Enums stored as `String`:** `repo_ref.default_tool` / `direction.tool` ∈ `"claude" | "codex" | "opencode" | "none"`; `direction_repo.role` ∈ `"write" | "read"`; `thread.status` ∈ `"active" | "paused" | "archived"`; `session.status` ∈ `"starting" | "running" | "exited"`.
- **Branch format:** `ws/<workspace.slug>/<thread.slug>/<direction.slug>`.
- **Worktree path:** `<worktree_home>/<workspace.slug>/<thread.slug>/<direction.slug>/<repo.slug>` where `worktree_home = ~/.weft/worktrees`.
- **DB file:** `~/.weft/weft.db`.

---

## Task 1: Project paths + dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/paths.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod paths;`)

- [ ] **Step 1: Add dependencies**

In `src-tauri/Cargo.toml` under `[dependencies]`, append:

```toml
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-rustls", "macros"] }
sea-orm-migration = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-rustls"] }
dirs = "5"
futures = "0.3"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

- [ ] **Step 2: Write the failing test for the path helpers**

Create `src-tauri/src/paths.rs`:

```rust
//! Canonical weft home + derived paths. Everything persistent lives under
//! ~/.weft so worktree cwds stay stable across restarts (resume depends on it).

use std::path::PathBuf;

/// ~/.weft, created if missing.
pub fn weft_home() -> std::io::Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no home dir"))?;
    let dir = home.join(".weft");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// ~/.weft/weft.db
pub fn db_path() -> std::io::Result<PathBuf> {
    Ok(weft_home()?.join("weft.db"))
}

/// ~/.weft/worktrees
pub fn worktree_home() -> std::io::Result<PathBuf> {
    let dir = weft_home()?.join("worktrees");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_under_weft_home() {
        let home = weft_home().unwrap();
        assert!(home.ends_with(".weft"));
        assert!(db_path().unwrap().ends_with("weft.db"));
        assert!(worktree_home().unwrap().ends_with("worktrees"));
    }
}
```

Add `mod paths;` to `src-tauri/src/lib.rs` (after the existing `mod` lines).

- [ ] **Step 3: Run the test to verify it compiles and passes**

Run: `cd src-tauri && cargo test paths:: 2>&1 | tail -15`
Expected: `paths_are_under_weft_home ... ok` (deps download on first run via the rsproxy mirror).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/paths.rs src-tauri/src/lib.rs
git commit -m "feat(M2): add SeaORM deps + weft home path helpers"
```

---

## Task 2: Slug module (filesystem- and git-ref-safe identifiers)

**Files:**
- Create: `src-tauri/src/slug.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod slug;`)

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/slug.rs`:

```rust
//! Slugs used in both filesystem paths and git branch names. Must be safe for
//! both: lowercase ASCII, digits, single hyphens; no leading/trailing hyphen;
//! never empty; de-duplicated against existing siblings.

/// Lowercase, replace any run of non-[a-z0-9] with a single '-', trim hyphens.
/// Empty input (or input with no usable chars) yields "item".
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in name.chars() {
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() {
            out.push(lc);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed
    }
}

/// slugify(name), then ensure uniqueness against `existing` by appending
/// "-2", "-3", ... until free.
pub fn unique_slug(name: &str, existing: &[String]) -> String {
    let base = slugify(name);
    if !existing.iter().any(|e| e == &base) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !existing.iter().any(|e| e == &candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("My Feature"), "my-feature");
        assert_eq!(slugify("web-app/.git"), "web-app-git");
        assert_eq!(slugify("  Hello   World  "), "hello-world");
        assert_eq!(slugify("café & co"), "caf-co");
        assert_eq!(slugify("!!!"), "item");
        assert_eq!(slugify(""), "item");
    }

    #[test]
    fn unique_slug_dedups() {
        let existing = vec!["api".to_string(), "api-2".to_string()];
        assert_eq!(unique_slug("API", &existing), "api-3");
        assert_eq!(unique_slug("fresh", &existing), "fresh");
    }
}
```

Add `mod slug;` to `src-tauri/src/lib.rs`.

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cd src-tauri && cargo test slug:: 2>&1 | tail -10`
Expected: `slugify_basic ... ok`, `unique_slug_dedups ... ok`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/slug.rs src-tauri/src/lib.rs
git commit -m "feat(M2): filesystem/git-ref-safe slug generation"
```

---

## Task 3: SeaORM entities

**Files:**
- Create: `src-tauri/src/store/mod.rs`, `src-tauri/src/store/entities/mod.rs`, and one file per entity.
- Modify: `src-tauri/src/lib.rs` (add `mod store;`)

- [ ] **Step 1: Create the entity files**

Create `src-tauri/src/store/entities/workspace.rs`:

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "workspace")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub slug: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Create `src-tauri/src/store/entities/repo_ref.rs`:

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "repo_ref")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub workspace_id: i32,
    pub name: String,
    pub slug: String,
    pub local_git_path: String,
    pub base_ref: String,
    pub default_tool: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Create `src-tauri/src/store/entities/thread.rs`:

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "thread")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub workspace_id: i32,
    pub title: String,
    pub slug: String,
    pub kind: String,   // "feature" | "bugfix" | "refactor" | "spike"
    pub status: String, // "active" | "paused" | "archived"
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Create `src-tauri/src/store/entities/direction.rs`:

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "direction")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub thread_id: i32,
    pub name: String,
    pub slug: String,
    pub tool: String,
    pub branch: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Create `src-tauri/src/store/entities/direction_repo.rs`:

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "direction_repo")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub direction_id: i32,
    pub repo_id: i32,
    pub role: String, // "write" | "read"
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Create `src-tauri/src/store/entities/worktree.rs`:

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "worktree")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub repo_id: i32,
    pub direction_id: i32,
    pub branch: String,
    pub path: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Create `src-tauri/src/store/entities/session.rs`:

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "session")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub direction_id: i32,
    pub repo_id: i32,
    pub tool: String,
    pub cwd: String,
    pub native_session_id: Option<String>,
    pub status: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

Create `src-tauri/src/store/entities/mod.rs`:

```rust
pub mod direction;
pub mod direction_repo;
pub mod repo_ref;
pub mod session;
pub mod thread;
pub mod worktree;
pub mod workspace;
```

Create a placeholder `src-tauri/src/store/mod.rs` (filled in Task 4):

```rust
pub mod entities;
```

Add `mod store;` to `src-tauri/src/lib.rs`.

- [ ] **Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build 2>&1 | tail -15`
Expected: `Finished` with no errors (warnings about unused entities are fine for now).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/store src-tauri/src/lib.rs
git commit -m "feat(M2): SeaORM entities for the workspace model"
```

---

## Task 4: Migration + Db connect

**Files:**
- Create: `src-tauri/src/store/migration/mod.rs`
- Modify: `src-tauri/src/store/mod.rs`

- [ ] **Step 1: Write the migration**

Create `src-tauri/src/store/migration/mod.rs`:

```rust
use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(M0001Init)]
    }
}

pub struct M0001Init;

impl MigrationName for M0001Init {
    fn name(&self) -> &str {
        "m0001_init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for M0001Init {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE workspace (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  name TEXT NOT NULL, slug TEXT NOT NULL, created_at TEXT NOT NULL
                );
                CREATE TABLE repo_ref (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  workspace_id INTEGER NOT NULL, name TEXT NOT NULL, slug TEXT NOT NULL,
                  local_git_path TEXT NOT NULL, base_ref TEXT NOT NULL, default_tool TEXT NOT NULL
                );
                CREATE TABLE thread (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  workspace_id INTEGER NOT NULL, title TEXT NOT NULL, slug TEXT NOT NULL,
                  kind TEXT NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL
                );
                CREATE TABLE direction (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  thread_id INTEGER NOT NULL, name TEXT NOT NULL, slug TEXT NOT NULL,
                  tool TEXT NOT NULL, branch TEXT NOT NULL, created_at TEXT NOT NULL
                );
                CREATE TABLE direction_repo (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  direction_id INTEGER NOT NULL, repo_id INTEGER NOT NULL, role TEXT NOT NULL
                );
                CREATE TABLE worktree (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  repo_id INTEGER NOT NULL, direction_id INTEGER NOT NULL,
                  branch TEXT NOT NULL, path TEXT NOT NULL, created_at TEXT NOT NULL
                );
                CREATE TABLE session (
                  id INTEGER PRIMARY KEY AUTOINCREMENT,
                  direction_id INTEGER NOT NULL, repo_id INTEGER NOT NULL, tool TEXT NOT NULL,
                  cwd TEXT NOT NULL, native_session_id TEXT, status TEXT NOT NULL, created_at TEXT NOT NULL
                );
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
```

- [ ] **Step 2: Write the Db connect helper + a connection test**

Replace `src-tauri/src/store/mod.rs` with:

```rust
pub mod entities;
pub mod migration;
pub mod repo;

use migration::Migrator;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

/// A connected, migrated database handle. Cheap to clone (Arc inside).
#[derive(Clone)]
pub struct Db(pub DatabaseConnection);

impl Db {
    /// Connect to a sqlite URL (e.g. "sqlite://<path>?mode=rwc" or
    /// "sqlite::memory:") and run migrations.
    pub async fn connect(url: &str) -> Result<Self, sea_orm::DbErr> {
        let conn = Database::connect(url).await?;
        Migrator::up(&conn, None).await?;
        Ok(Db(conn))
    }

    /// Connect to the on-disk weft db (~/.weft/weft.db).
    pub async fn open_default() -> anyhow::Result<Self> {
        let path = crate::paths::db_path()?;
        let url = format!("sqlite://{}?mode=rwc", path.to_string_lossy());
        Ok(Self::connect(&url).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connects_and_migrates_in_memory() {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        // a table from the migration must exist
        use sea_orm::ConnectionTrait;
        db.0
            .execute_unprepared("SELECT id FROM workspace LIMIT 0")
            .await
            .unwrap();
    }
}
```

Create an empty `src-tauri/src/store/repo.rs` for now:

```rust
//! Repository functions. Filled in Task 5.
```

- [ ] **Step 3: Run the migration test**

Run: `cd src-tauri && cargo test store::tests::connects 2>&1 | tail -15`
Expected: `connects_and_migrates_in_memory ... ok`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/store
git commit -m "feat(M2): schema migration + Db connect (in-memory + on-disk)"
```

---

## Task 5: Repository functions (CRUD + cascade delete)

**Files:**
- Modify: `src-tauri/src/store/repo.rs`

- [ ] **Step 1: Write the repository functions**

Replace `src-tauri/src/store/repo.rs` with:

```rust
//! All DB reads/writes go through here. Keeps SeaORM specifics out of commands.

use super::entities::{direction, direction_repo, repo_ref, session, thread, worktree, workspace};
use super::Db;
use crate::slug::unique_slug;
use anyhow::Result;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

fn now() -> String {
    // RFC3339 without pulling chrono: seconds since epoch is enough for ordering.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{secs}")
}

pub async fn create_workspace(db: &Db, name: &str) -> Result<workspace::Model> {
    let existing: Vec<String> = workspace::Entity::find()
        .all(&db.0)
        .await?
        .into_iter()
        .map(|w| w.slug)
        .collect();
    let m = workspace::ActiveModel {
        name: Set(name.to_string()),
        slug: Set(unique_slug(name, &existing)),
        created_at: Set(now()),
        ..Default::default()
    };
    Ok(m.insert(&db.0).await?)
}

pub async fn list_workspaces(db: &Db) -> Result<Vec<workspace::Model>> {
    Ok(workspace::Entity::find().all(&db.0).await?)
}

pub async fn add_repo_ref(
    db: &Db,
    workspace_id: i32,
    name: &str,
    local_git_path: &str,
    base_ref: &str,
    default_tool: &str,
) -> Result<repo_ref::Model> {
    let existing: Vec<String> = repo_ref::Entity::find()
        .filter(repo_ref::Column::WorkspaceId.eq(workspace_id))
        .all(&db.0)
        .await?
        .into_iter()
        .map(|r| r.slug)
        .collect();
    let m = repo_ref::ActiveModel {
        workspace_id: Set(workspace_id),
        name: Set(name.to_string()),
        slug: Set(unique_slug(name, &existing)),
        local_git_path: Set(local_git_path.to_string()),
        base_ref: Set(base_ref.to_string()),
        default_tool: Set(default_tool.to_string()),
        ..Default::default()
    };
    Ok(m.insert(&db.0).await?)
}

pub async fn create_thread(
    db: &Db,
    workspace_id: i32,
    title: &str,
    kind: &str,
) -> Result<thread::Model> {
    let existing: Vec<String> = thread::Entity::find()
        .filter(thread::Column::WorkspaceId.eq(workspace_id))
        .all(&db.0)
        .await?
        .into_iter()
        .map(|t| t.slug)
        .collect();
    let m = thread::ActiveModel {
        workspace_id: Set(workspace_id),
        title: Set(title.to_string()),
        slug: Set(unique_slug(title, &existing)),
        kind: Set(kind.to_string()),
        status: Set("active".to_string()),
        created_at: Set(now()),
        ..Default::default()
    };
    Ok(m.insert(&db.0).await?)
}

pub async fn list_threads(db: &Db, workspace_id: i32) -> Result<Vec<thread::Model>> {
    Ok(thread::Entity::find()
        .filter(thread::Column::WorkspaceId.eq(workspace_id))
        .all(&db.0)
        .await?)
}

/// Create a direction with its per-repo scope. `scope` is (repo_id, role).
pub async fn create_direction(
    db: &Db,
    thread_id: i32,
    name: &str,
    tool: &str,
    scope: &[(i32, String)],
) -> Result<direction::Model> {
    let t = thread::Entity::find_by_id(thread_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("thread {thread_id} not found"))?;
    let ws = workspace::Entity::find_by_id(t.workspace_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("workspace missing"))?;
    let existing: Vec<String> = direction::Entity::find()
        .filter(direction::Column::ThreadId.eq(thread_id))
        .all(&db.0)
        .await?
        .into_iter()
        .map(|d| d.slug)
        .collect();
    let slug = unique_slug(name, &existing);
    let branch = format!("ws/{}/{}/{}", ws.slug, t.slug, slug);
    let dir = direction::ActiveModel {
        thread_id: Set(thread_id),
        name: Set(name.to_string()),
        slug: Set(slug),
        tool: Set(tool.to_string()),
        branch: Set(branch),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(&db.0)
    .await?;
    for (repo_id, role) in scope {
        direction_repo::ActiveModel {
            direction_id: Set(dir.id),
            repo_id: Set(*repo_id),
            role: Set(role.clone()),
            ..Default::default()
        }
        .insert(&db.0)
        .await?;
    }
    Ok(dir)
}

pub async fn direction_write_repos(db: &Db, direction_id: i32) -> Result<Vec<repo_ref::Model>> {
    let links = direction_repo::Entity::find()
        .filter(direction_repo::Column::DirectionId.eq(direction_id))
        .filter(direction_repo::Column::Role.eq("write"))
        .all(&db.0)
        .await?;
    let mut out = Vec::new();
    for l in links {
        if let Some(r) = repo_ref::Entity::find_by_id(l.repo_id).one(&db.0).await? {
            out.push(r);
        }
    }
    Ok(out)
}

pub async fn record_worktree(
    db: &Db,
    repo_id: i32,
    direction_id: i32,
    branch: &str,
    path: &str,
) -> Result<worktree::Model> {
    Ok(worktree::ActiveModel {
        repo_id: Set(repo_id),
        direction_id: Set(direction_id),
        branch: Set(branch.to_string()),
        path: Set(path.to_string()),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(&db.0)
    .await?)
}

pub async fn list_worktrees(db: &Db, direction_id: Option<i32>) -> Result<Vec<worktree::Model>> {
    let q = worktree::Entity::find();
    let q = match direction_id {
        Some(id) => q.filter(worktree::Column::DirectionId.eq(id)),
        None => q,
    };
    Ok(q.all(&db.0).await?)
}

pub async fn worktree_for(
    db: &Db,
    direction_id: i32,
    repo_id: i32,
) -> Result<Option<worktree::Model>> {
    Ok(worktree::Entity::find()
        .filter(worktree::Column::DirectionId.eq(direction_id))
        .filter(worktree::Column::RepoId.eq(repo_id))
        .one(&db.0)
        .await?)
}

/// Delete a thread and everything under it. Returns the worktree paths that the
/// caller must physically remove via git (DB rows are gone after this).
pub async fn delete_thread_cascade(db: &Db, thread_id: i32) -> Result<Vec<(i32, String)>> {
    let dirs = direction::Entity::find()
        .filter(direction::Column::ThreadId.eq(thread_id))
        .all(&db.0)
        .await?;
    let mut removed: Vec<(i32, String)> = Vec::new(); // (repo_id, worktree path)
    for d in &dirs {
        let wts = worktree::Entity::find()
            .filter(worktree::Column::DirectionId.eq(d.id))
            .all(&db.0)
            .await?;
        for w in wts {
            removed.push((w.repo_id, w.path.clone()));
            worktree::Entity::delete_by_id(w.id).exec(&db.0).await?;
        }
        session::Entity::delete_many()
            .filter(session::Column::DirectionId.eq(d.id))
            .exec(&db.0)
            .await?;
        direction_repo::Entity::delete_many()
            .filter(direction_repo::Column::DirectionId.eq(d.id))
            .exec(&db.0)
            .await?;
        direction::Entity::delete_by_id(d.id).exec(&db.0).await?;
    }
    thread::Entity::delete_by_id(thread_id).exec(&db.0).await?;
    Ok(removed)
}

pub async fn create_session(
    db: &Db,
    direction_id: i32,
    repo_id: i32,
    tool: &str,
    cwd: &str,
) -> Result<session::Model> {
    Ok(session::ActiveModel {
        direction_id: Set(direction_id),
        repo_id: Set(repo_id),
        tool: Set(tool.to_string()),
        cwd: Set(cwd.to_string()),
        native_session_id: Set(None),
        status: Set("starting".to_string()),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(&db.0)
    .await?)
}

pub async fn set_session_native_id(db: &Db, session_id: i32, native_id: &str) -> Result<()> {
    if let Some(s) = session::Entity::find_by_id(session_id).one(&db.0).await? {
        let mut a: session::ActiveModel = s.into();
        a.native_session_id = Set(Some(native_id.to_string()));
        a.status = Set("running".to_string());
        a.update(&db.0).await?;
    }
    Ok(())
}

pub async fn get_session(db: &Db, session_id: i32) -> Result<Option<session::Model>> {
    Ok(session::Entity::find_by_id(session_id).one(&db.0).await?)
}
```

- [ ] **Step 2: Write the failing test for create + cascade**

Append to `src-tauri/src/store/repo.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Db;

    async fn mem() -> Db {
        Db::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn create_and_cascade_delete() {
        let db = mem().await;
        let ws = create_workspace(&db, "Demo WS").await.unwrap();
        assert_eq!(ws.slug, "demo-ws");
        let repo = add_repo_ref(&db, ws.id, "web-app", "/tmp/x", "main", "claude")
            .await
            .unwrap();
        let t = create_thread(&db, ws.id, "Add login", "feature")
            .await
            .unwrap();
        let dir = create_direction(&db, t.id, "main", "claude", &[(repo.id, "write".into())])
            .await
            .unwrap();
        assert_eq!(dir.branch, "ws/demo-ws/add-login/main");

        // pretend it was materialized
        record_worktree(&db, repo.id, dir.id, &dir.branch, "/tmp/wt")
            .await
            .unwrap();
        assert_eq!(list_worktrees(&db, Some(dir.id)).await.unwrap().len(), 1);
        assert!(direction_write_repos(&db, dir.id).await.unwrap().len() == 1);

        // cascade delete returns the path to clean and empties the rows
        let removed = delete_thread_cascade(&db, t.id).await.unwrap();
        assert_eq!(removed, vec![(repo.id, "/tmp/wt".to_string())]);
        assert_eq!(list_workspaces(&db).await.unwrap().len(), 1); // ws survives
        assert_eq!(list_threads(&db, ws.id).await.unwrap().len(), 0);
        assert_eq!(list_worktrees(&db, None).await.unwrap().len(), 0);
    }
}
```

- [ ] **Step 3: Run the test to verify it passes**

Run: `cd src-tauri && cargo test store::repo::tests::create_and_cascade 2>&1 | tail -20`
Expected: `create_and_cascade_delete ... ok`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/store/repo.rs
git commit -m "feat(M2): repository fns — CRUD, scope, cascade delete"
```

---

## Task 6: git worktree extensions (list + diff)

**Files:**
- Modify: `src-tauri/src/git.rs`

- [ ] **Step 1: Write the new git helpers**

Append to `src-tauri/src/git.rs` (the `git()` helper already exists in the file from M1):

```rust
use serde::Serialize;

/// One file's diff stat in a worktree.
#[derive(Serialize, Debug, PartialEq)]
pub struct FileDiff {
    pub path: String,
    pub added: u32,
    pub removed: u32,
}

/// Per-repo working-tree diff stat (staged + unstaged + untracked-as-added).
#[derive(Serialize, Debug, Default)]
pub struct DiffSummary {
    pub files: Vec<FileDiff>,
}

/// `git worktree list --porcelain` parsed into (path, branch) pairs.
pub fn list_worktrees(repo: &Path) -> Result<Vec<(String, String)>> {
    let out = git(repo, &["worktree", "list", "--porcelain"])?;
    let mut res = Vec::new();
    let mut path: Option<String> = None;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            path = Some(p.to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            if let Some(p) = path.take() {
                let branch = b.strip_prefix("refs/heads/").unwrap_or(b).to_string();
                res.push((p, branch));
            }
        }
    }
    Ok(res)
}

/// Diff stat for a worktree: tracked changes via `git diff --numstat HEAD`
/// plus untracked files counted as fully-added.
pub fn repo_diff(worktree_path: &Path) -> Result<DiffSummary> {
    let mut files = Vec::new();
    let numstat = git(worktree_path, &["diff", "--numstat", "HEAD"])?;
    for line in numstat.lines() {
        let mut parts = line.split('\t');
        let added = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let removed = parts.next().unwrap_or("0").parse().unwrap_or(0);
        if let Some(path) = parts.next() {
            files.push(FileDiff { path: path.to_string(), added, removed });
        }
    }
    let untracked = git(
        worktree_path,
        &["ls-files", "--others", "--exclude-standard"],
    )?;
    for path in untracked.lines().filter(|l| !l.is_empty()) {
        let full = worktree_path.join(path);
        let added = std::fs::read_to_string(&full)
            .map(|c| c.lines().count() as u32)
            .unwrap_or(0);
        files.push(FileDiff { path: path.to_string(), added, removed: 0 });
    }
    Ok(DiffSummary { files })
}
```

- [ ] **Step 2: Write the failing integration test**

Create `src-tauri/tests/m2_git.rs`:

```rust
//! git worktree + diff helpers against a real throwaway repo.
use std::path::PathBuf;
use std::process::Command;

fn sh(dir: &PathBuf, args: &[&str]) {
    let st = Command::new(args[0]).args(&args[1..]).current_dir(dir).status().unwrap();
    assert!(st.success(), "cmd {:?} failed", args);
}

#[test]
fn worktree_list_and_diff() {
    let root = std::env::temp_dir().join(format!("weft-m2-git-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo = root.join("repo");
    let wt = root.join("wt");
    std::fs::create_dir_all(&repo).unwrap();
    sh(&repo, &["git", "init", "-q"]);
    sh(&repo, &["git", "config", "user.email", "t@t.t"]);
    sh(&repo, &["git", "config", "user.name", "t"]);
    std::fs::write(repo.join("README.md"), "# x\n").unwrap();
    sh(&repo, &["git", "add", "-A"]);
    sh(&repo, &["git", "commit", "-q", "-m", "init"]);
    sh(&repo, &["git", "worktree", "add", "-q", "-b", "ws/d/t/m", wt.to_str().unwrap()]);

    // new untracked file in the worktree
    std::fs::write(wt.join("hello.txt"), "a\nb\n").unwrap();

    let wts = weft_app_lib::git::list_worktrees(&repo).unwrap();
    assert!(wts.iter().any(|(_, b)| b == "ws/d/t/m"));

    let diff = weft_app_lib::git::repo_diff(&wt).unwrap();
    let hello = diff.files.iter().find(|f| f.path == "hello.txt").unwrap();
    assert_eq!(hello.added, 2);

    let _ = std::fs::remove_dir_all(&root);
}
```

Note: this requires `git`'s functions and `FileDiff` to be `pub` and reachable as `weft_app_lib::git::...`. Ensure `pub mod git;` in `lib.rs` (M1 has `mod git;` — change to `pub mod git;`). Do the same for any module the tests reach (`pub mod store;`, `pub mod materialize;`, `pub mod slug;`, `pub mod paths;`).

- [ ] **Step 3: Run the integration test**

Run: `cd src-tauri && cargo test --test m2_git 2>&1 | tail -15`
Expected: `worktree_list_and_diff ... ok`.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/git.rs src-tauri/src/lib.rs src-tauri/tests/m2_git.rs
git commit -m "feat(M2): git worktree listing + per-repo diff stat"
```

---

## Task 7: Materialize a direction into worktrees

**Files:**
- Create: `src-tauri/src/materialize.rs`
- Modify: `src-tauri/src/lib.rs` (add `pub mod materialize;`)

- [ ] **Step 1: Write the materialize function**

Create `src-tauri/src/materialize.rs`:

```rust
//! Turn a direction's write-scope into git worktrees under the persistent
//! worktree home, and record them. Read-scope mounting is M5; none-scope is
//! never touched. Nothing is written into the canonical repo (architecture §2.1).

use crate::store::{entities, repo, Db};
use crate::{git, paths};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// For each write repo in `direction_id`, create a worktree at
/// `<worktree_home>/<ws>/<thread>/<direction>/<repo>` on the direction's branch
/// and record it. Idempotent: existing worktree rows/paths are reused.
pub async fn materialize_direction(
    db: &Db,
    direction_id: i32,
) -> Result<Vec<entities::worktree::Model>> {
    use sea_orm::EntityTrait;
    let dir = entities::direction::Entity::find_by_id(direction_id)
        .one(&db.0)
        .await?
        .context("direction not found")?;
    let thread = entities::thread::Entity::find_by_id(dir.thread_id)
        .one(&db.0)
        .await?
        .context("thread not found")?;
    let ws = entities::workspace::Entity::find_by_id(thread.workspace_id)
        .one(&db.0)
        .await?
        .context("workspace not found")?;

    let home = paths::worktree_home()?;
    let mut out = Vec::new();
    for repo_ref in repo::direction_write_repos(db, direction_id).await? {
        if let Some(existing) = repo::worktree_for(db, direction_id, repo_ref.id).await? {
            out.push(existing);
            continue;
        }
        let path: PathBuf = home
            .join(&ws.slug)
            .join(&thread.slug)
            .join(&dir.slug)
            .join(&repo_ref.slug);
        git::add_worktree(
            std::path::Path::new(&repo_ref.local_git_path),
            &dir.branch,
            &path,
        )
        .with_context(|| format!("worktree for repo {}", repo_ref.name))?;
        let rec = repo::record_worktree(
            db,
            repo_ref.id,
            direction_id,
            &dir.branch,
            &path.to_string_lossy(),
        )
        .await?;
        out.push(rec);
    }
    Ok(out)
}

/// Physically remove worktrees (called during cascade delete). `removed` is the
/// (repo_id, path) list returned by `repo::delete_thread_cascade`.
pub async fn cleanup_worktrees(db: &Db, removed: &[(i32, String)]) -> Result<()> {
    use sea_orm::EntityTrait;
    for (repo_id, path) in removed {
        if let Some(r) = entities::repo_ref::Entity::find_by_id(*repo_id)
            .one(&db.0)
            .await?
        {
            let _ = git::remove_worktree(
                std::path::Path::new(&r.local_git_path),
                std::path::Path::new(path),
            );
        }
    }
    Ok(())
}
```

Add `pub mod materialize;` to `lib.rs`. (`remove_worktree` already exists in `git.rs` from M1; drop its `#[allow(dead_code)]` now that it's used.)

- [ ] **Step 2: Write the failing integration test (acceptance scenarios ①②③)**

Create `src-tauri/tests/m2_worktree.rs`:

```rust
//! M2 acceptance: two directions don't interfere; delete cleans worktrees;
//! same repo across two threads doesn't collide.
use std::path::{Path, PathBuf};
use std::process::Command;
use weft_app_lib::materialize::{cleanup_worktrees, materialize_direction};
use weft_app_lib::store::{repo, Db};

fn sh(dir: &Path, args: &[&str]) {
    let st = Command::new(args[0]).args(&args[1..]).current_dir(dir).status().unwrap();
    assert!(st.success(), "cmd {:?} failed", args);
}

fn make_repo(root: &Path, name: &str) -> PathBuf {
    let p = root.join(name);
    std::fs::create_dir_all(&p).unwrap();
    sh(&p, &["git", "init", "-q"]);
    sh(&p, &["git", "config", "user.email", "t@t.t"]);
    sh(&p, &["git", "config", "user.name", "t"]);
    std::fs::write(p.join("README.md"), "# x\n").unwrap();
    sh(&p, &["git", "add", "-A"]);
    sh(&p, &["git", "commit", "-q", "-m", "init"]);
    p
}

#[tokio::test]
async fn m2_acceptance() {
    let root = std::env::temp_dir().join(format!("weft-m2-acc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let repo_a = make_repo(&root, "repo-a");
    let repo_b = make_repo(&root, "repo-b");

    let db = Db::connect("sqlite::memory:").await.unwrap();
    let ws = repo::create_workspace(&db, "ws").await.unwrap();
    let ra = repo::add_repo_ref(&db, ws.id, "repo-a", repo_a.to_str().unwrap(), "main", "claude").await.unwrap();
    let rb = repo::add_repo_ref(&db, ws.id, "repo-b", repo_b.to_str().unwrap(), "main", "claude").await.unwrap();

    // ① one thread, two directions on different repos -> independent worktrees
    let t1 = repo::create_thread(&db, ws.id, "t1", "feature").await.unwrap();
    let d1 = repo::create_direction(&db, t1.id, "da", "claude", &[(ra.id, "write".into())]).await.unwrap();
    let d2 = repo::create_direction(&db, t1.id, "db", "claude", &[(rb.id, "write".into())]).await.unwrap();
    let w1 = materialize_direction(&db, d1.id).await.unwrap();
    let w2 = materialize_direction(&db, d2.id).await.unwrap();
    assert_eq!(w1.len(), 1);
    assert_eq!(w2.len(), 1);
    assert!(Path::new(&w1[0].path).exists());
    assert!(Path::new(&w2[0].path).exists());
    assert_ne!(w1[0].path, w2[0].path);

    // ③ same repo across two threads -> two worktrees, distinct branches/paths
    let t2 = repo::create_thread(&db, ws.id, "t2", "feature").await.unwrap();
    let d3 = repo::create_direction(&db, t2.id, "da", "claude", &[(ra.id, "write".into())]).await.unwrap();
    let w3 = materialize_direction(&db, d3.id).await.unwrap();
    assert_ne!(w3[0].path, w1[0].path, "same repo, different thread -> different path");
    assert_ne!(w3[0].branch, w1[0].branch, "branches must differ");
    // both worktrees coexist in repo-a
    let listed = weft_app_lib::git::list_worktrees(&repo_a).unwrap();
    assert!(listed.iter().any(|(_, b)| b == &w1[0].branch));
    assert!(listed.iter().any(|(_, b)| b == &w3[0].branch));

    // ② delete a thread -> its worktrees are gone (rows + on disk), others remain
    let removed = repo::delete_thread_cascade(&db, t1.id).await.unwrap();
    cleanup_worktrees(&db, &removed).await.unwrap();
    assert!(!Path::new(&w1[0].path).exists(), "deleted worktree removed from disk");
    assert!(Path::new(&w3[0].path).exists(), "other thread's worktree survives");
    assert_eq!(repo::list_worktrees(&db, None).await.unwrap().len(), 1);

    let _ = std::fs::remove_dir_all(&root);
}
```

- [ ] **Step 3: Run the acceptance test**

Run: `cd src-tauri && cargo test --test m2_worktree 2>&1 | tail -25`
Expected: `m2_acceptance ... ok` (all three scenarios pass).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/materialize.rs src-tauri/src/lib.rs src-tauri/src/git.rs src-tauri/tests/m2_worktree.rs
git commit -m "feat(M2): materialize directions into persistent worktrees + acceptance tests"
```

---

## Task 8: Multi-session PtyState + open_session(direction, repo)

**Files:**
- Modify: `src-tauri/src/pty.rs`

This refactors M1's single-session `PtyState` to a keyed map and makes `open_session` operate on a materialized worktree. The spawn/capture/frame-batch internals from M1 are reused; only the state shape, the command signatures, and the session-id sink change.

- [ ] **Step 1: Change `PtyState` to a keyed map and thread the `Db`**

In `src-tauri/src/pty.rs`, replace the `PtyState` struct and `Active` map usage:

```rust
use std::collections::HashMap;

#[derive(Default)]
pub struct PtyState {
    sessions: Mutex<HashMap<i32, Active>>,
}
```

Delete the old single-session fields (`active`, `cwd`, `repo`, `branch`, `session_id`). Per-session metadata now lives in the DB `session` row.

- [ ] **Step 2: Rewrite `open_session` to take ids and use the materialized worktree**

Replace the M1 `open_session` / `open_session_impl` with:

```rust
use crate::store::{repo, Db};

#[derive(serde::Serialize, Clone)]
pub struct SessionInfo {
    pub session_id: i32,
    pub repo: String,
    pub worktree: String,
    pub branch: String,
    pub tool: String,
    pub resumed: bool,
}

#[tauri::command]
pub async fn open_session(
    app: AppHandle,
    db: State<'_, Db>,
    state: State<'_, PtyState>,
    direction_id: i32,
    repo_id: i32,
) -> Result<SessionInfo, String> {
    open_session_impl(app, &db, &state, direction_id, repo_id)
        .await
        .map_err(|e| e.to_string())
}

async fn open_session_impl(
    app: AppHandle,
    db: &Db,
    state: &PtyState,
    direction_id: i32,
    repo_id: i32,
) -> anyhow::Result<SessionInfo> {
    let wt = repo::worktree_for(db, direction_id, repo_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("no materialized worktree for that direction+repo"))?;
    let dir = {
        use sea_orm::EntityTrait;
        crate::store::entities::direction::Entity::find_by_id(direction_id)
            .one(&db.0)
            .await?
            .ok_or_else(|| anyhow::anyhow!("direction not found"))?
    };
    let cwd = std::path::PathBuf::from(&wt.path);
    let sess = repo::create_session(db, direction_id, repo_id, &dir.tool, &wt.path).await?;

    let active = spawn(&app, &cwd, None, sess.id, db.clone())?;
    state.sessions.lock().unwrap().insert(sess.id, active);

    Ok(SessionInfo {
        session_id: sess.id,
        repo: wt.path.clone(),
        worktree: wt.path,
        branch: wt.branch,
        tool: dir.tool,
        resumed: false,
    })
}
```

- [ ] **Step 3: Update `spawn` to carry `session_id` + `Db` and persist the captured native id**

Change the `spawn` signature to `fn spawn(app: &AppHandle, cwd: &PathBuf, resume_id: Option<&str>, session_id: i32, db: Db) -> Result<Active>`. In the capture thread (the M1 poll loop that emits `session://id`), replace the `app.try_state::<PtyState>()` write with a DB write and an event keyed by session:

```rust
if let Some(id) = claude::capture_session_id(&dir, t0) {
    let _ = app.emit(SESSION_ID_EVENT, serde_json::json!({ "sessionId": session_id, "nativeId": id }));
    let db = db.clone();
    let id2 = id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = repo::set_session_native_id(&db, session_id, &id2).await;
    });
    break;
}
```

(The capture thread is a plain OS thread; use `tauri::async_runtime::spawn` to run the async DB write. `Db` is `Clone`.)

- [ ] **Step 4: Make `write_pty` / `resize_pty` / `kill_session` / `resume_session` key by `session_id`**

```rust
#[tauri::command]
pub fn write_pty(state: State<PtyState>, session_id: i32, data: String) -> Result<(), String> {
    let mut g = state.sessions.lock().unwrap();
    let a = g.get_mut(&session_id).ok_or("no such session")?;
    a.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    a.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resize_pty(state: State<PtyState>, session_id: i32, rows: u16, cols: u16) -> Result<(), String> {
    let g = state.sessions.lock().unwrap();
    if let Some(a) = g.get(&session_id) {
        a.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn kill_session(state: State<PtyState>, session_id: i32) -> Result<(), String> {
    if let Some(mut a) = state.sessions.lock().unwrap().remove(&session_id) {
        a.alive.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = a.child.kill();
        let _ = a.child.wait();
    }
    Ok(())
}

#[tauri::command]
pub async fn resume_session(
    app: AppHandle,
    db: State<'_, Db>,
    state: State<'_, PtyState>,
    session_id: i32,
) -> Result<SessionInfo, String> {
    resume_impl(app, &db, &state, session_id).await.map_err(|e| e.to_string())
}

async fn resume_impl(app: AppHandle, db: &Db, state: &PtyState, session_id: i32) -> anyhow::Result<SessionInfo> {
    let s = repo::get_session(db, session_id).await?.ok_or_else(|| anyhow::anyhow!("no session"))?;
    let native = s.native_session_id.clone().ok_or_else(|| anyhow::anyhow!("native id not captured yet"))?;
    let wt = repo::worktree_for(db, s.direction_id, s.repo_id).await?.ok_or_else(|| anyhow::anyhow!("worktree gone"))?;
    // kill the old live process if present
    if let Some(mut a) = state.sessions.lock().unwrap().remove(&session_id) {
        a.alive.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = a.child.kill();
        let _ = a.child.wait();
    }
    let cwd = std::path::PathBuf::from(&wt.path);
    let active = spawn(&app, &cwd, Some(&native), session_id, db.clone())?;
    state.sessions.lock().unwrap().insert(session_id, active);
    Ok(SessionInfo {
        session_id,
        repo: wt.path.clone(),
        worktree: wt.path,
        branch: wt.branch,
        tool: s.tool,
        resumed: true,
    })
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cd src-tauri && cargo build 2>&1 | tail -20`
Expected: `Finished` (no errors).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/pty.rs
git commit -m "feat(M2): multi-session PtyState keyed by session id; open/resume on materialized worktrees"
```

---

## Task 9: Tauri command surface + state wiring

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Thin command wrappers over the store + materialize**

Create `src-tauri/src/commands.rs`:

```rust
//! Tauri command surface for the M2 workspace model. Thin wrappers; all logic
//! lives in store::repo and materialize.

use crate::store::{entities, repo, Db};
use crate::materialize;
use tauri::State;

type R<T> = Result<T, String>;
fn e<E: ToString>(x: E) -> String { x.to_string() }

#[tauri::command]
pub async fn create_workspace(db: State<'_, Db>, name: String) -> R<entities::workspace::Model> {
    repo::create_workspace(&db, &name).await.map_err(e)
}

#[tauri::command]
pub async fn list_workspaces(db: State<'_, Db>) -> R<Vec<entities::workspace::Model>> {
    repo::list_workspaces(&db).await.map_err(e)
}

#[tauri::command]
pub async fn add_repo_ref(
    db: State<'_, Db>,
    workspace_id: i32,
    name: String,
    local_git_path: String,
) -> R<entities::repo_ref::Model> {
    if !crate::git::is_git_repo(std::path::Path::new(&local_git_path)) {
        return Err("not a git repository".into());
    }
    // default base ref = current branch of the repo
    let base = crate::git::current_branch(std::path::Path::new(&local_git_path)).unwrap_or_else(|_| "main".into());
    repo::add_repo_ref(&db, workspace_id, &name, &local_git_path, &base, "claude").await.map_err(e)
}

#[tauri::command]
pub async fn create_thread(db: State<'_, Db>, workspace_id: i32, title: String, kind: String) -> R<entities::thread::Model> {
    repo::create_thread(&db, workspace_id, &title, &kind).await.map_err(e)
}

#[tauri::command]
pub async fn list_threads(db: State<'_, Db>, workspace_id: i32) -> R<Vec<entities::thread::Model>> {
    repo::list_threads(&db, workspace_id).await.map_err(e)
}

/// scope: list of { repoId, role } from the frontend.
#[derive(serde::Deserialize)]
pub struct ScopeItem { pub repo_id: i32, pub role: String }

#[tauri::command]
pub async fn create_direction(
    db: State<'_, Db>,
    thread_id: i32,
    name: String,
    tool: String,
    scope: Vec<ScopeItem>,
) -> R<entities::direction::Model> {
    let scope: Vec<(i32, String)> = scope.into_iter().map(|s| (s.repo_id, s.role)).collect();
    let dir = repo::create_direction(&db, thread_id, &name, &tool, &scope).await.map_err(e)?;
    materialize::materialize_direction(&db, dir.id).await.map_err(e)?;
    Ok(dir)
}

#[tauri::command]
pub async fn list_worktrees(db: State<'_, Db>, direction_id: Option<i32>) -> R<Vec<entities::worktree::Model>> {
    repo::list_worktrees(&db, direction_id).await.map_err(e)
}

#[tauri::command]
pub async fn repo_diff(db: State<'_, Db>, worktree_id: i32) -> R<crate::git::DiffSummary> {
    use sea_orm::EntityTrait;
    let w = entities::worktree::Entity::find_by_id(worktree_id).one(&db.0).await.map_err(e)?
        .ok_or("worktree not found")?;
    crate::git::repo_diff(std::path::Path::new(&w.path)).map_err(e)
}

#[tauri::command]
pub async fn delete_thread(db: State<'_, Db>, thread_id: i32) -> R<()> {
    let removed = repo::delete_thread_cascade(&db, thread_id).await.map_err(e)?;
    materialize::cleanup_worktrees(&db, &removed).await.map_err(e)
}
```

- [ ] **Step 2: Add `current_branch` to git.rs**

Append to `src-tauri/src/git.rs`:

```rust
/// Current branch name of a repo (e.g. "main").
pub fn current_branch(repo: &Path) -> Result<String> {
    git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
}
```

- [ ] **Step 3: Wire state + commands in lib.rs**

Replace `src-tauri/src/lib.rs` `run()` body to manage the `Db` and register all commands. Keep the debug MCP-bridge plugin block:

```rust
pub mod paths;
pub mod slug;
pub mod store;
pub mod git;
pub mod materialize;
mod batch;
mod claude;
mod pty;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Open the DB synchronously before building the app.
    let db = tauri::async_runtime::block_on(async {
        store::Db::open_default().await.expect("open weft.db")
    });

    let mut builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    builder
        .manage(db)
        .manage(pty::PtyState::default())
        .invoke_handler(tauri::generate_handler![
            commands::create_workspace,
            commands::list_workspaces,
            commands::add_repo_ref,
            commands::create_thread,
            commands::list_threads,
            commands::create_direction,
            commands::list_worktrees,
            commands::repo_diff,
            commands::delete_thread,
            pty::open_session,
            pty::resume_session,
            pty::write_pty,
            pty::resize_pty,
            pty::kill_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo build 2>&1 | tail -20`
Expected: `Finished` (no errors).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/git.rs src-tauri/src/lib.rs
git commit -m "feat(M2): Tauri command surface + Db state wiring"
```

---

## Task 10: Headless end-to-end via the dev MCP bridge

**Files:** none (verification only). Drives the running dev app through `window.__TAURI_INTERNALS__.invoke`, the same harness used to verify M1.

- [ ] **Step 1: Launch the dev app**

Run (background): `cd /Users/solojiang/workspace/weft && PATH=/Users/solojiang/.nvm/versions/node/v24.15.0/bin:$HOME/.cargo/bin:$PATH npm run tauri dev`
Wait for `WebSocket server listening on: 0.0.0.0:9223`, then `driver_session(start, port 9223)`.

- [ ] **Step 2: Drive the full model via IPC and assert**

Using `mcp__tauri__webview_execute_js` with `window.__TAURI_INTERNALS__.invoke`, in sequence (use a real git repo path on disk for `add_repo_ref`, e.g. a throwaway `git init` under `/private/tmp`):

```js
// create workspace -> repo -> thread -> two directions (write) -> list worktrees
(async () => {
  const inv = window.__TAURI_INTERNALS__.invoke;
  const ws = await inv('create_workspace', { name: 'E2E' });
  const r = await inv('add_repo_ref', { workspaceId: ws.id, name: 'demo', localGitPath: '/private/tmp/weft-e2e-repo' });
  const t = await inv('create_thread', { workspaceId: ws.id, title: 'feat', kind: 'feature' });
  const d = await inv('create_direction', { threadId: t.id, name: 'main', tool: 'claude', scope: [{ repoId: r.id, role: 'write' }] });
  const wts = await inv('list_worktrees', { directionId: d.id });
  return { ws, r, t, d, wts };
})()
```

Assert: `wts.length === 1`, the worktree `path` is under `~/.weft/worktrees/e2e/feat/main/demo`, branch is `ws/e2e/feat/main`. Confirm on disk with Bash (`ls` the path, `git -C <repo> worktree list`).

- [ ] **Step 3: Open + resume a session on the materialized worktree**

```js
(async () => {
  const inv = window.__TAURI_INTERNALS__.invoke;
  const s = await inv('open_session', { directionId: <d.id>, repoId: <r.id> });
  return s; // { session_id, worktree, branch, ... }
})()
```

Drive trust + a file-creating prompt + approval via `write_pty` (`{ sessionId: <id>, data: '1\r' }` etc.), confirm the file appears in the worktree, then `resume_session` (`{ sessionId: <id> }`) and confirm the same native jsonl is reused (Bash: line count grows, single file) — exactly the M1 checks, now keyed by session id.

- [ ] **Step 4: Delete the thread and confirm cleanup**

```js
(async () => { return await window.__TAURI_INTERNALS__.invoke('delete_thread', { threadId: <t.id> }); })()
```

Assert with Bash: the worktree path no longer exists, `git -C <repo> worktree list` no longer shows the branch, and `list_worktrees` returns `[]`.

- [ ] **Step 5: Commit the verification notes**

Update the M2 spec's "Done" section with the e2e result, then:

```bash
git add docs/superpowers/specs/2026-06-05-m2-data-model-worktree-orchestration-design.md
git commit -m "docs(M2): record headless e2e verification result"
```

---

## Self-review checklist (run before handoff)

- **Spec coverage:** data model (T3,T4,T5) ✓; persistent worktree home (T1,T7) ✓; branch namespacing (T5) ✓; per-repo diff (T6,T9) ✓; multi-session pty + open on worktree (T8) ✓; commands (T9) ✓; the 3 acceptance scenarios (T7) ✓; headless e2e (T10) ✓. Read-scope mounting and dependency linking are explicitly Out of M2.
- **Placeholder scan:** none — every step has real code/commands.
- **Type consistency:** `Db`, `repo::*` signatures, `entities::*::Model`, `SessionInfo { session_id, repo, worktree, branch, tool, resumed }`, `DiffSummary/FileDiff`, `current_branch`, `is_git_repo` (M1), `add_worktree`/`remove_worktree` (M1) are referenced consistently across tasks.

## Notes for the executor

- All cargo builds go through the rsproxy mirror (`~/.cargo/config.toml`), so first-time SeaORM compiles are tolerable but not instant.
- `node` must be v24 for `npm run tauri dev` (vite 7). Use the explicit PATH prefix shown in Task 10.
- The M1 single-session frontend (`src/App.tsx`, `TerminalPanel.tsx`) still calls the old `open_session` shape; it will break against the new command signatures. That's expected — the polished nav UI that consumes the new commands is the follow-on `$impeccable craft workspace-nav` pass, not part of M2. If you want the dev app to stay launchable mid-M2, leave the M1 frontend calling a temporary hardcoded `directionId/repoId` or skip the frontend until the craft pass.
