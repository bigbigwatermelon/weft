//! All DB reads/writes go through here. Keeps SeaORM specifics out of commands.

use super::entities::{
    app_setting, direction, im_route, lead_message, session, skill_enable, skill_source, thread,
    workspace,
};
use super::Db;
use crate::slug::unique_slug;
use anyhow::Result;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set, TryIntoModel,
};

fn now() -> String {
    // RFC3339 without pulling chrono: seconds since epoch is enough for ordering.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

/// Unix-secs as string, for skill_source.last_synced.
pub fn now_unix() -> String {
    now()
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

/// Rename = display-name only. slug is a stable identifier and never changes.
pub async fn rename_workspace(db: &Db, workspace_id: i32, name: &str) -> Result<workspace::Model> {
    let name = validate_display_name(name, "workspace name")?;
    let dup = workspace::Entity::find()
        .filter(workspace::Column::Name.eq(name))
        .filter(workspace::Column::Id.ne(workspace_id))
        .one(&db.0)
        .await?;
    if dup.is_some() {
        anyhow::bail!("another workspace already named {name:?}");
    }
    let m = workspace::Entity::find_by_id(workspace_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("workspace {workspace_id} not found"))?;
    let mut a: workspace::ActiveModel = m.into();
    a.name = Set(name.to_string());
    Ok(a.update(&db.0).await?)
}

/// Trim and reject empty for any display field. Centralized so rename helpers
/// stay consistent and error wording can evolve in one place.
fn validate_display_name<'a>(input: &'a str, what: &str) -> Result<&'a str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{what} cannot be empty");
    }
    Ok(trimmed)
}

/// The most-recently created workspace (highest id), if any. Used as the
/// default-workspace bootstrap target for first-run onboarding.
pub async fn latest_workspace(db: &Db) -> Result<Option<workspace::Model>> {
    Ok(workspace::Entity::find()
        .order_by_desc(workspace::Column::Id)
        .one(&db.0)
        .await?)
}

pub async fn add_skill_source(
    db: &Db,
    git_url: &str,
    git_ref: Option<&str>,
) -> Result<skill_source::Model> {
    let ref_norm = git_ref.unwrap_or("").to_string();
    // Idempotent: same (url, ref) reuses the existing row so repeat clicks /
    // re-imports don't pile up duplicate clones under ~/.atlas/skills/sources/.
    // A *different* ref on the same URL is still a distinct source.
    if let Some(existing) = skill_source::Entity::find()
        .filter(skill_source::Column::GitUrl.eq(git_url))
        .filter(skill_source::Column::GitRef.eq(&ref_norm))
        .one(&db.0)
        .await?
    {
        return Ok(existing);
    }
    let m = skill_source::ActiveModel {
        git_url: Set(git_url.to_string()),
        git_ref: Set(ref_norm),
        last_synced: Set(String::new()),
        last_status: Set("never".to_string()),
        ..Default::default()
    };
    Ok(m.insert(&db.0).await?)
}

pub async fn list_skill_sources(db: &Db) -> Result<Vec<skill_source::Model>> {
    Ok(skill_source::Entity::find().all(&db.0).await?)
}

pub async fn get_skill_source(db: &Db, id: i32) -> Result<Option<skill_source::Model>> {
    Ok(skill_source::Entity::find_by_id(id).one(&db.0).await?)
}

pub async fn set_skill_source_status(
    db: &Db,
    id: i32,
    status: &str,
    synced: Option<&str>,
) -> Result<()> {
    if let Some(m) = skill_source::Entity::find_by_id(id).one(&db.0).await? {
        let mut a: skill_source::ActiveModel = m.into();
        a.last_status = Set(status.to_string());
        if let Some(s) = synced {
            a.last_synced = Set(s.to_string());
        }
        a.update(&db.0).await?;
    }
    Ok(())
}

pub async fn remove_skill_source(db: &Db, id: i32) -> Result<()> {
    skill_enable::Entity::delete_many()
        .filter(skill_enable::Column::SourceId.eq(id))
        .exec(&db.0)
        .await?;
    skill_source::Entity::delete_by_id(id).exec(&db.0).await?;
    Ok(())
}

pub async fn set_skill_enable(
    db: &Db,
    source_id: i32,
    skill_name: &str,
    scope: &str,
    on: bool,
) -> Result<()> {
    let existing = skill_enable::Entity::find()
        .filter(skill_enable::Column::SourceId.eq(source_id))
        .filter(skill_enable::Column::SkillName.eq(skill_name))
        .filter(skill_enable::Column::Scope.eq(scope))
        .one(&db.0)
        .await?;
    match (on, existing) {
        (true, None) => {
            let m = skill_enable::ActiveModel {
                source_id: Set(source_id),
                skill_name: Set(skill_name.to_string()),
                scope: Set(scope.to_string()),
                ..Default::default()
            };
            m.insert(&db.0).await?;
        }
        (false, Some(m)) => {
            skill_enable::Entity::delete_by_id(m.id).exec(&db.0).await?;
        }
        _ => {}
    }
    Ok(())
}

pub async fn list_skill_enable(db: &Db) -> Result<Vec<skill_enable::Model>> {
    Ok(skill_enable::Entity::find().all(&db.0).await?)
}

pub async fn get_setting(db: &Db, key: &str) -> Result<Option<String>> {
    Ok(app_setting::Entity::find_by_id(key)
        .one(&db.0)
        .await?
        .map(|m| m.value))
}

pub async fn set_setting(db: &Db, key: &str, value: &str) -> Result<()> {
    let m = app_setting::ActiveModel {
        key: Set(key.to_string()),
        value: Set(value.to_string()),
    };
    app_setting::Entity::insert(m)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(app_setting::Column::Key)
                .update_column(app_setting::Column::Value)
                .to_owned(),
        )
        .exec(&db.0)
        .await?;
    Ok(())
}

/// Workspace container used by per-IM-conversation Concierge threads.
pub const K_CONCIERGE_WORKSPACE: &str = "concierge.workspace_id";

pub async fn create_thread(
    db: &Db,
    workspace_id: i32,
    title: &str,
    kind: &str,
    lead_tool: &str,
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
        lead_tool: Set(lead_tool.to_string()),
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

pub async fn get_thread(db: &Db, thread_id: i32) -> Result<Option<thread::Model>> {
    Ok(thread::Entity::find_by_id(thread_id).one(&db.0).await?)
}

/// Display-title only; slug stays (see rename_workspace).
pub async fn rename_thread(db: &Db, thread_id: i32, title: &str) -> Result<thread::Model> {
    let title = validate_display_name(title, "task title")?;
    let m = thread::Entity::find_by_id(thread_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("thread {thread_id} not found"))?;
    let dup = thread::Entity::find()
        .filter(thread::Column::WorkspaceId.eq(m.workspace_id))
        .filter(thread::Column::Title.eq(title))
        .filter(thread::Column::Id.ne(thread_id))
        .one(&db.0)
        .await?;
    if dup.is_some() {
        anyhow::bail!("another task in this workspace already titled {title:?}");
    }
    let mut a: thread::ActiveModel = m.into();
    a.title = Set(title.to_string());
    Ok(a.update(&db.0).await?)
}

pub async fn list_directions(db: &Db, thread_id: i32) -> Result<Vec<direction::Model>> {
    Ok(direction::Entity::find()
        .filter(direction::Column::ThreadId.eq(thread_id))
        .all(&db.0)
        .await?)
}

pub async fn create_direction(
    db: &Db,
    thread_id: i32,
    name: &str,
    tool: &str,
    mandate: &str,
) -> Result<direction::Model> {
    let _thread = thread::Entity::find_by_id(thread_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("thread {thread_id} not found"))?;
    let existing: Vec<String> = direction::Entity::find()
        .filter(direction::Column::ThreadId.eq(thread_id))
        .all(&db.0)
        .await?
        .into_iter()
        .map(|d| d.slug)
        .collect();
    let slug = unique_slug(name, &existing);
    let dir = direction::ActiveModel {
        thread_id: Set(thread_id),
        name: Set(name.to_string()),
        slug: Set(slug),
        tool: Set(tool.to_string()),
        status: Set("queued".to_string()),
        mandate: Set(normalize_mandate(mandate).to_string()),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(&db.0)
    .await?;
    Ok(dir)
}

/// Anything that isn't explicitly "impl-only" is the default "plan+impl".
pub fn normalize_mandate(m: &str) -> &'static str {
    if m == "impl-only" {
        "impl-only"
    } else {
        "plan+impl"
    }
}

pub async fn get_direction(db: &Db, direction_id: i32) -> Result<Option<direction::Model>> {
    Ok(direction::Entity::find_by_id(direction_id)
        .one(&db.0)
        .await?)
}

/// Set a direction's lifecycle status (agent- or human-driven). No-op if gone.
pub async fn set_direction_status(db: &Db, direction_id: i32, status: &str) -> Result<()> {
    if let Some(d) = direction::Entity::find_by_id(direction_id)
        .one(&db.0)
        .await?
    {
        let mut a: direction::ActiveModel = d.into();
        a.status = Set(status.to_string());
        a.update(&db.0).await?;
    }
    Ok(())
}

/// Display-name only; slug stays stable.
pub async fn rename_direction(db: &Db, direction_id: i32, name: &str) -> Result<direction::Model> {
    let name = validate_display_name(name, "task name")?;
    let m = direction::Entity::find_by_id(direction_id)
        .one(&db.0)
        .await?
        .ok_or_else(|| anyhow::anyhow!("direction {direction_id} not found"))?;
    let dup = direction::Entity::find()
        .filter(direction::Column::ThreadId.eq(m.thread_id))
        .filter(direction::Column::Name.eq(name))
        .filter(direction::Column::Id.ne(direction_id))
        .one(&db.0)
        .await?;
    if dup.is_some() {
        anyhow::bail!("another run in this task already named {name:?}");
    }
    let mut a: direction::ActiveModel = m.into();
    a.name = Set(name.to_string());
    Ok(a.update(&db.0).await?)
}

pub async fn delete_thread_cascade(db: &Db, thread_id: i32) -> Result<()> {
    let dirs = direction::Entity::find()
        .filter(direction::Column::ThreadId.eq(thread_id))
        .all(&db.0)
        .await?;
    for d in &dirs {
        session::Entity::delete_many()
            .filter(session::Column::DirectionId.eq(d.id))
            .exec(&db.0)
            .await?;
        direction::Entity::delete_by_id(d.id).exec(&db.0).await?;
    }
    thread::Entity::delete_by_id(thread_id).exec(&db.0).await?;
    Ok(())
}

pub async fn create_session(
    db: &Db,
    direction_id: i32,
    tool: &str,
    cwd: &str,
) -> Result<session::Model> {
    create_session_with_computer_use(db, direction_id, tool, cwd, false).await
}

pub async fn create_session_with_computer_use(
    db: &Db,
    direction_id: i32,
    tool: &str,
    cwd: &str,
    computer_use_enabled: bool,
) -> Result<session::Model> {
    Ok(session::ActiveModel {
        direction_id: Set(direction_id),
        tool: Set(tool.to_string()),
        cwd: Set(cwd.to_string()),
        native_session_id: Set(None),
        computer_use_enabled: Set(computer_use_enabled),
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

/// The most-recent session row for a direction, by insertion order.
/// Used to decide resume-vs-fresh when no live PTY is tracked in memory.
pub async fn latest_session_for(db: &Db, direction_id: i32) -> Result<Option<session::Model>> {
    Ok(session::Entity::find()
        .filter(session::Column::DirectionId.eq(direction_id))
        .order_by_desc(session::Column::Id)
        .one(&db.0)
        .await?)
}

/// The most-recent session row for a direction — the coordinator's route from a
/// bus wake target to its chat engine.
pub async fn latest_session_for_direction(
    db: &Db,
    direction_id: i32,
) -> Result<Option<session::Model>> {
    Ok(session::Entity::find()
        .filter(session::Column::DirectionId.eq(direction_id))
        .order_by_desc(session::Column::Id)
        .one(&db.0)
        .await?)
}

// ---- chat timeline (lead console + chat-mode workers) ----

#[allow(clippy::too_many_arguments)]
pub async fn insert_lead_message(
    db: &Db,
    thread_id: i32,
    session_id: Option<i32>,
    turn_id: i32,
    role: &str,
    kind: &str,
    content: &str,
    status: &str,
) -> Result<lead_message::Model> {
    Ok(lead_message::ActiveModel {
        thread_id: Set(thread_id),
        session_id: Set(session_id),
        turn_id: Set(turn_id),
        role: Set(role.to_string()),
        kind: Set(kind.to_string()),
        content: Set(content.to_string()),
        status: Set(status.to_string()),
        created_at: Set(now()),
        ..Default::default()
    }
    .insert(&db.0)
    .await?)
}

pub async fn update_lead_message(db: &Db, id: i32, content: &str, status: &str) -> Result<()> {
    if let Some(m) = lead_message::Entity::find_by_id(id).one(&db.0).await? {
        let mut a: lead_message::ActiveModel = m.into();
        a.content = Set(content.to_string());
        a.status = Set(status.to_string());
        a.update(&db.0).await?;
    }
    Ok(())
}

pub async fn list_lead_messages(db: &Db, thread_id: i32) -> Result<Vec<lead_message::Model>> {
    Ok(lead_message::Entity::find()
        .filter(lead_message::Column::ThreadId.eq(thread_id))
        .order_by_asc(lead_message::Column::Id)
        .all(&db.0)
        .await?)
}

/// The next turn number for a thread's timeline (1-based).
pub async fn next_turn_id(db: &Db, thread_id: i32) -> Result<i32> {
    Ok(list_lead_messages(db, thread_id)
        .await?
        .iter()
        .map(|m| m.turn_id)
        .max()
        .unwrap_or(0)
        + 1)
}

/// Flip the OLDEST queued user message to complete — called when the engine
/// flushes the front of its FIFO into the process; queue order equals row
/// insertion order, so position (not content) is the identity. `_text` kept
/// for telemetry/debug call sites.
pub async fn complete_queued(db: &Db, thread_id: i32, _text: &str) -> Result<()> {
    if let Some(m) = lead_message::Entity::find()
        .filter(lead_message::Column::ThreadId.eq(thread_id))
        .filter(lead_message::Column::Status.eq("queued"))
        .order_by_asc(lead_message::Column::Id)
        .one(&db.0)
        .await?
    {
        let mut a: lead_message::ActiveModel = m.into();
        a.status = Set("complete".to_string());
        a.update(&db.0).await?;
    }
    Ok(())
}

/// The lead's persisted engine metadata (native session id) lives in a single
/// role=system kind=meta row per thread, invisible to the timeline UI.
async fn lead_meta_row(db: &Db, thread_id: i32) -> Result<Option<lead_message::Model>> {
    Ok(lead_message::Entity::find()
        .filter(lead_message::Column::ThreadId.eq(thread_id))
        .filter(lead_message::Column::Kind.eq("meta"))
        .one(&db.0)
        .await?)
}

fn lead_meta_map(content: &str) -> serde_json::Map<String, serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(content)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

async fn set_lead_meta_field(
    db: &Db,
    thread_id: i32,
    key: &str,
    value: serde_json::Value,
) -> Result<()> {
    let existing = lead_meta_row(db, thread_id).await?;
    let mut map = existing
        .as_ref()
        .map(|m| lead_meta_map(&m.content))
        .unwrap_or_default();
    map.insert(key.to_string(), value);
    let content = serde_json::Value::Object(map).to_string();
    match existing {
        Some(m) => {
            let mut a: lead_message::ActiveModel = m.into();
            a.content = Set(content);
            a.update(&db.0).await?;
        }
        None => {
            insert_lead_message(
                db, thread_id, None, 0, "system", "meta", &content, "complete",
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn lead_native_id(db: &Db, thread_id: i32) -> Result<Option<String>> {
    Ok(lead_meta_row(db, thread_id).await?.and_then(|m| {
        serde_json::from_str::<serde_json::Value>(&m.content)
            .ok()?
            .get("native_id")?
            .as_str()
            .map(String::from)
    }))
}

pub async fn set_lead_native_id(db: &Db, thread_id: i32, native_id: &str) -> Result<()> {
    set_lead_meta_field(db, thread_id, "native_id", serde_json::json!(native_id)).await
}

pub async fn lead_computer_use_enabled(db: &Db, thread_id: i32) -> Result<Option<bool>> {
    Ok(lead_meta_row(db, thread_id).await?.and_then(|m| {
        serde_json::from_str::<serde_json::Value>(&m.content)
            .ok()?
            .get("computer_use_enabled")?
            .as_bool()
    }))
}

pub async fn set_lead_computer_use_enabled(db: &Db, thread_id: i32, enabled: bool) -> Result<()> {
    set_lead_meta_field(
        db,
        thread_id,
        "computer_use_enabled",
        serde_json::json!(enabled),
    )
    .await
}

// ─────────────────────────── im_route (M2) ───────────────────────────

/// Bind an task (thread) to an IM thread. Upserts on `thread_id`: re-binding the
/// same task replaces its target. Returns the resulting row.
pub async fn bind_im_route(
    db: &Db,
    thread_id: i32,
    channel: &str,
    chat_id: &str,
    im_thread_ref: &str,
) -> Result<im_route::Model> {
    if let Some(existing) = im_route::Entity::find()
        .filter(im_route::Column::ThreadId.eq(thread_id))
        .one(&db.0)
        .await?
    {
        let mut a: im_route::ActiveModel = existing.into();
        a.channel = Set(channel.to_string());
        a.chat_id = Set(chat_id.to_string());
        a.im_thread_ref = Set(im_thread_ref.to_string());
        let m = a.update(&db.0).await?;
        return Ok(m);
    }
    let now = now();
    let am = im_route::ActiveModel {
        channel: Set(channel.to_string()),
        chat_id: Set(chat_id.to_string()),
        im_thread_ref: Set(im_thread_ref.to_string()),
        thread_id: Set(thread_id),
        created_at: Set(now),
        ..Default::default()
    };
    let m = am.insert(&db.0).await?.try_into_model()?;
    Ok(m)
}

pub async fn unbind_im_route(db: &Db, thread_id: i32) -> Result<()> {
    im_route::Entity::delete_many()
        .filter(im_route::Column::ThreadId.eq(thread_id))
        .exec(&db.0)
        .await?;
    Ok(())
}

pub async fn list_im_routes(db: &Db) -> Result<Vec<im_route::Model>> {
    Ok(im_route::Entity::find().all(&db.0).await?)
}

pub async fn im_route_of_thread(db: &Db, thread_id: i32) -> Result<Option<im_route::Model>> {
    Ok(im_route::Entity::find()
        .filter(im_route::Column::ThreadId.eq(thread_id))
        .one(&db.0)
        .await?)
}

/// Reverse lookup: which task is this IM thread bound to?
pub async fn im_route_of_thread_ref(
    db: &Db,
    channel: &str,
    chat_id: &str,
    im_thread_ref: &str,
) -> Result<Option<im_route::Model>> {
    Ok(im_route::Entity::find()
        .filter(im_route::Column::Channel.eq(channel))
        .filter(im_route::Column::ChatId.eq(chat_id))
        .filter(im_route::Column::ImThreadRef.eq(im_thread_ref))
        .one(&db.0)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Db;

    async fn mem() -> Db {
        Db::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn lead_message_roundtrip() {
        let db = mem().await;
        let m = insert_lead_message(
            &db,
            1,
            None,
            1,
            "user",
            "text",
            r#"{"text":"hi"}"#,
            "complete",
        )
        .await
        .unwrap();
        assert_eq!(m.thread_id, 1);
        update_lead_message(&db, m.id, r#"{"text":"hi!"}"#, "complete")
            .await
            .unwrap();
        let all = list_lead_messages(&db, 1).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].content, r#"{"text":"hi!"}"#);
        assert_eq!(next_turn_id(&db, 1).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn queued_flips_to_complete() {
        let db = mem().await;
        insert_lead_message(
            &db,
            2,
            None,
            2,
            "user",
            "text",
            r#"{"text":"later"}"#,
            "queued",
        )
        .await
        .unwrap();
        complete_queued(&db, 2, "later").await.unwrap();
        let all = list_lead_messages(&db, 2).await.unwrap();
        assert_eq!(all[0].status, "complete");
    }

    #[tokio::test]
    async fn lead_native_id_upserts() {
        let db = mem().await;
        assert!(lead_native_id(&db, 3).await.unwrap().is_none());
        assert_eq!(lead_computer_use_enabled(&db, 3).await.unwrap(), None);
        set_lead_computer_use_enabled(&db, 3, true).await.unwrap();
        set_lead_native_id(&db, 3, "abc").await.unwrap();
        set_lead_native_id(&db, 3, "def").await.unwrap();
        assert_eq!(
            lead_native_id(&db, 3).await.unwrap().as_deref(),
            Some("def")
        );
        assert_eq!(lead_computer_use_enabled(&db, 3).await.unwrap(), Some(true));
        // meta row stays single + out of turn numbering
        assert_eq!(list_lead_messages(&db, 3).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn im_route_bind_and_lookup() {
        let db = mem().await;
        let r = bind_im_route(&db, 7, "feishu", "oc_chat", "th_1")
            .await
            .unwrap();
        assert_eq!(r.thread_id, 7);
        // forward lookup by thread_id
        let got = im_route_of_thread(&db, 7).await.unwrap().unwrap();
        assert_eq!(got.im_thread_ref, "th_1");
        // reverse lookup by (channel, chat_id, im_thread_ref)
        let got = im_route_of_thread_ref(&db, "feishu", "oc_chat", "th_1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.thread_id, 7);
        // re-bind same task: row count stays 1, target replaced
        bind_im_route(&db, 7, "feishu", "oc_chat", "th_2")
            .await
            .unwrap();
        assert_eq!(list_im_routes(&db).await.unwrap().len(), 1);
        assert!(im_route_of_thread_ref(&db, "feishu", "oc_chat", "th_1")
            .await
            .unwrap()
            .is_none());
        // unbind
        unbind_im_route(&db, 7).await.unwrap();
        assert!(im_route_of_thread(&db, 7).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn im_route_thread_ref_is_unique_across_tasks() {
        // Same (channel, chat_id, im_thread_ref) cannot bind to two different tasks.
        let db = mem().await;
        bind_im_route(&db, 1, "feishu", "oc_chat", "th_1")
            .await
            .unwrap();
        let err = bind_im_route(&db, 2, "feishu", "oc_chat", "th_1").await;
        assert!(err.is_err(), "second bind should violate unique index");
    }

    #[tokio::test]
    async fn create_and_cascade_delete() {
        let db = mem().await;
        let ws = create_workspace(&db, "Demo WS").await.unwrap();
        assert_eq!(ws.slug, "demo-ws");
        let t = create_thread(&db, ws.id, "Add login", "feature", "claude")
            .await
            .unwrap();
        let dir = create_direction(&db, t.id, "main", "claude", "plan+impl")
            .await
            .unwrap();
        let _session = create_session(&db, dir.id, "claude", "/tmp/run")
            .await
            .unwrap();

        delete_thread_cascade(&db, t.id).await.unwrap();
        assert_eq!(list_workspaces(&db).await.unwrap().len(), 1); // ws survives
        assert_eq!(list_threads(&db, ws.id).await.unwrap().len(), 0);
        assert!(latest_session_for(&db, dir.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn latest_session_for_returns_newest_with_native() {
        let db = mem().await;
        let ws = create_workspace(&db, "Demo WS").await.unwrap();
        let thread = create_thread(&db, ws.id, "T", "feature", "claude")
            .await
            .unwrap();
        let dir = create_direction(&db, thread.id, "D", "claude", "impl-only")
            .await
            .unwrap();
        // older session (no native), then newer (native captured)
        let _s1 = create_session(&db, dir.id, "claude", "/tmp/x")
            .await
            .unwrap();
        let s2 = create_session(&db, dir.id, "claude", "/tmp/x")
            .await
            .unwrap();
        set_session_native_id(&db, s2.id, "abc-123").await.unwrap();

        let latest = latest_session_for(&db, dir.id).await.unwrap().unwrap();
        assert_eq!(latest.id, s2.id);
        assert_eq!(latest.native_session_id.as_deref(), Some("abc-123"));
    }

    #[tokio::test]
    async fn session_records_computer_use_creation_intent() {
        let db = mem().await;
        let ws = create_workspace(&db, "Demo WS").await.unwrap();
        let thread = create_thread(&db, ws.id, "T", "feature", "claude")
            .await
            .unwrap();
        let dir = create_direction(&db, thread.id, "D", "claude", "impl-only")
            .await
            .unwrap();

        let default_session = create_session(&db, dir.id, "claude", "/tmp/a")
            .await
            .unwrap();
        let enabled_session =
            create_session_with_computer_use(&db, dir.id, "claude", "/tmp/b", true)
                .await
                .unwrap();

        assert!(!default_session.computer_use_enabled);
        assert!(enabled_session.computer_use_enabled);
        let latest = latest_session_for(&db, dir.id).await.unwrap().unwrap();
        assert_eq!(latest.id, enabled_session.id);
        assert!(latest.computer_use_enabled);
    }

    #[tokio::test]
    async fn direction_can_back_a_generic_session() {
        let db = Db::connect("sqlite::memory:").await.unwrap();
        let ws = create_workspace(&db, "People Ops").await.unwrap();
        let t = create_thread(&db, ws.id, "Draft offer email", "task", "codex")
            .await
            .unwrap();

        let d = create_direction(&db, t.id, "Main run", "codex", "plan+impl")
            .await
            .unwrap();

        let s = create_session(&db, d.id, "codex", "/tmp/atlas-run")
            .await
            .unwrap();
        let latest = latest_session_for(&db, d.id).await.unwrap().unwrap();
        assert_eq!(latest.id, s.id);
        assert_eq!(latest.cwd, "/tmp/atlas-run");
    }

    #[tokio::test]
    async fn create_thread_stamps_lead_tool() {
        let db = mem().await;
        let ws = create_workspace(&db, "w").await.unwrap();
        let t = create_thread(&db, ws.id, "Add feature", "feature", "codex")
            .await
            .unwrap();
        assert_eq!(t.lead_tool, "codex");
    }

    #[tokio::test]
    async fn app_setting_roundtrip() {
        let db = mem().await;
        assert_eq!(get_setting(&db, "default_tool").await.unwrap(), None);
        set_setting(&db, "default_tool", "codex").await.unwrap();
        assert_eq!(
            get_setting(&db, "default_tool").await.unwrap(),
            Some("codex".to_string())
        );
        // Overwrite, not duplicate.
        set_setting(&db, "default_tool", "claude").await.unwrap();
        assert_eq!(
            get_setting(&db, "default_tool").await.unwrap(),
            Some("claude".to_string())
        );
    }

    #[tokio::test]
    async fn rename_updates_display_name_only() {
        let db = mem().await;
        let ws = create_workspace(&db, "Demo WS").await.unwrap();
        let t = create_thread(&db, ws.id, "Add login", "feature", "claude")
            .await
            .unwrap();
        let d = create_direction(&db, t.id, "main", "claude", "plan+impl")
            .await
            .unwrap();

        // trim + only display fields change; slugs remain stable.
        let ws2 = rename_workspace(&db, ws.id, "  New WS  ").await.unwrap();
        assert_eq!(ws2.name, "New WS");
        assert_eq!(ws2.slug, "demo-ws");

        let t2 = rename_thread(&db, t.id, "Add SSO login").await.unwrap();
        assert_eq!(t2.title, "Add SSO login");
        assert_eq!(t2.slug, "add-login");

        let d2 = rename_direction(&db, d.id, "api work").await.unwrap();
        assert_eq!(d2.name, "api work");
        assert_eq!(d2.slug, "main");
    }

    #[tokio::test]
    async fn rename_rejects_empty_and_missing() {
        let db = mem().await;
        let ws = create_workspace(&db, "w").await.unwrap();
        assert!(rename_workspace(&db, ws.id, "   ").await.is_err());
        assert!(rename_workspace(&db, 9999, "x").await.is_err());
        assert!(rename_thread(&db, 9999, "x").await.is_err());
        assert!(rename_direction(&db, 9999, "x").await.is_err());
    }

    #[tokio::test]
    async fn rename_rejects_sibling_collisions() {
        let db = mem().await;
        let ws_a = create_workspace(&db, "Alpha").await.unwrap();
        let ws_b = create_workspace(&db, "Beta").await.unwrap();
        // same name as another workspace → rejected; renaming to its own
        // current name is a no-op-style allowed (filtered by id-ne).
        assert!(rename_workspace(&db, ws_b.id, "Alpha").await.is_err());
        assert!(rename_workspace(&db, ws_a.id, "Alpha").await.is_ok());

        let t1 = create_thread(&db, ws_a.id, "Login", "feature", "claude")
            .await
            .unwrap();
        let t2 = create_thread(&db, ws_a.id, "Signup", "feature", "claude")
            .await
            .unwrap();
        // duplicate within same workspace → rejected
        assert!(rename_thread(&db, t2.id, "Login").await.is_err());
        // same title in a DIFFERENT workspace is fine
        let t3 = create_thread(&db, ws_b.id, "Other", "feature", "claude")
            .await
            .unwrap();
        assert!(rename_thread(&db, t3.id, "Login").await.is_ok());

        let d1 = create_direction(&db, t1.id, "api", "claude", "plan+impl")
            .await
            .unwrap();
        let d2 = create_direction(&db, t1.id, "ui", "claude", "plan+impl")
            .await
            .unwrap();
        assert!(rename_direction(&db, d2.id, "api").await.is_err());
        // same direction name under a DIFFERENT thread is fine
        let d3 = create_direction(&db, t2.id, "main", "claude", "plan+impl")
            .await
            .unwrap();
        assert!(rename_direction(&db, d3.id, "api").await.is_ok());
        let _ = d1;
    }

    #[tokio::test]
    async fn skill_source_and_enable_roundtrip() {
        let db = mem().await;
        let s = add_skill_source(&db, "https://example.com/skills.git", None)
            .await
            .unwrap();
        assert_eq!(s.git_url, "https://example.com/skills.git");
        assert_eq!(s.last_status, "never");
        // update status
        set_skill_source_status(&db, s.id, "ok", Some("123"))
            .await
            .unwrap();
        let got = list_skill_sources(&db).await.unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].last_status, "ok");
        assert_eq!(got[0].last_synced, "123");
        // enable a skill globally, then list
        set_skill_enable(&db, s.id, "deploy", "global", true)
            .await
            .unwrap();
        let en = list_skill_enable(&db).await.unwrap();
        assert_eq!(en.len(), 1);
        assert_eq!(
            (en[0].skill_name.as_str(), en[0].scope.as_str()),
            ("deploy", "global")
        );
        // toggling off removes it
        set_skill_enable(&db, s.id, "deploy", "global", false)
            .await
            .unwrap();
        assert!(list_skill_enable(&db).await.unwrap().is_empty());
        // remove source cascades its enables
        set_skill_enable(&db, s.id, "x", "ws:1", true)
            .await
            .unwrap();
        remove_skill_source(&db, s.id).await.unwrap();
        assert!(list_skill_sources(&db).await.unwrap().is_empty());
        assert!(list_skill_enable(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn add_skill_source_is_idempotent_on_same_url_ref() {
        let db = mem().await;
        let url = "https://example.com/skills.git";
        let a = add_skill_source(&db, url, None).await.unwrap();
        let b = add_skill_source(&db, url, None).await.unwrap();
        let c = add_skill_source(&db, url, Some("")).await.unwrap();
        assert_eq!(a.id, b.id, "same url+empty ref must reuse row");
        assert_eq!(a.id, c.id, "None and Some(\"\") must collapse");
        assert_eq!(list_skill_sources(&db).await.unwrap().len(), 1);

        // Different ref on same URL is a distinct source.
        let d = add_skill_source(&db, url, Some("main")).await.unwrap();
        assert_ne!(a.id, d.id);
        assert_eq!(list_skill_sources(&db).await.unwrap().len(), 2);
    }
}
