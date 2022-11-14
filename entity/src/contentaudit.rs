use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, Eq, PartialEq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum AuditResult {
    Failure = 0,
    Success = 1,
}

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content_audit")]
pub struct Model {
    #[sea_orm(primary_key, indexed)]
    pub id: i32,
    #[sea_orm(unique, indexed)]
    pub content_key: i32,
    pub created_at: DateTime<Utc>,
    pub result: AuditResult,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contentkey::Entity",
        from = "Column::ContentKey",
        to = "super::contentkey::Column::Id"
    )]
    ContentKey,
}

impl Related<super::contentkey::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
