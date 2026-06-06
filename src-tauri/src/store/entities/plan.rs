use sea_orm::entity::prelude::*;

/// The lead's proposed decomposition of a thread's Task into directions +
/// per-repo scope (ARCHITECTURE §4.10, §5.1). One per thread; the human
/// confirms (and may edit) it before directions are materialized.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[sea_orm(table_name = "plan")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub thread_id: i32,
    /// JSON: { rationale, directions: [{ name, tool, writes:[repo], reads:[repo] }] }.
    pub proposal: String,
    /// "proposed" | "confirmed"
    pub status: String,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
