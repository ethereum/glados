//! `SeaORM` Entity for sync_audit
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "sync_audit")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: SyncAuditStatus,
    pub segment_size: i32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum SyncAuditStatus {
    InProgress = 0,
    Completed = 1,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::sync_audit_segment::Entity")]
    SyncAuditSegment,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<super::sync_audit_segment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SyncAuditSegment.def()
    }
}

pub async fn create(segment_size: i32, conn: &DatabaseConnection) -> Result<Model> {
    let active = ActiveModel {
        id: NotSet,
        started_at: Set(Utc::now()),
        completed_at: Set(None),
        status: Set(SyncAuditStatus::InProgress),
        segment_size: Set(segment_size),
    };
    Ok(active.insert(conn).await?)
}

pub async fn get(id: i32, conn: &DatabaseConnection) -> Result<Option<Model>> {
    Ok(Entity::find_by_id(id).one(conn).await?)
}

pub async fn mark_complete(id: i32, conn: &DatabaseConnection) -> Result<()> {
    let audit = get(id, conn)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Audit not found"))?;
    let mut audit: ActiveModel = audit.into();
    audit.completed_at = Set(Some(Utc::now()));
    audit.status = Set(SyncAuditStatus::Completed);
    audit.update(conn).await?;
    Ok(())
}
