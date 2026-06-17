use crate::store::{repo, Db};
use anyhow::Result;

pub const K_ENABLED: &str = "computer_use_enabled";

pub async fn enabled(db: &Db) -> Result<bool> {
    Ok(matches!(
        repo::get_setting(db, K_ENABLED).await?.as_deref(),
        Some("1") | Some("true")
    ))
}

pub async fn set_enabled(db: &Db, on: bool) -> Result<()> {
    repo::set_setting(db, K_ENABLED, if on { "1" } else { "0" }).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn mem_db() -> Db {
        Db::connect("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn computer_use_enabled_defaults_false() {
        let db = mem_db().await;
        assert!(!enabled(&db).await.unwrap());
    }

    #[tokio::test]
    async fn computer_use_enabled_roundtrips() {
        let db = mem_db().await;
        set_enabled(&db, true).await.unwrap();
        assert!(enabled(&db).await.unwrap());
        set_enabled(&db, false).await.unwrap();
        assert!(!enabled(&db).await.unwrap());
    }
}
