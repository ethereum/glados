//! `SeaORM` Entity for sync_audit_error
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "sync_audit_error")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub sync_audit_segment_id: i32,
    pub block_number: i32,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Related<super::sync_audit_segment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncAuditSegment.def()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sync_audit_segment::Entity",
        from = "Column::SyncAuditSegmentId",
        to = "super::sync_audit_segment::Column::Id"
    )]
    SyncAuditSegment,
}

impl ActiveModelBehavior for ActiveModel {}

pub async fn create(
    sync_audit_segment_id: i32,
    block_number: i32,
    error_type: Option<String>,
    error_message: Option<String>,
    conn: &DatabaseConnection,
) -> Result<Model> {
    let active = ActiveModel {
        id: NotSet,
        sync_audit_segment_id: Set(sync_audit_segment_id),
        block_number: Set(block_number),
        error_type: Set(error_type),
        error_message: Set(error_message),
        created_at: Set(Utc::now()),
    };
    Ok(active.insert(conn).await?)
}
