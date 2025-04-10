//! `SeaORM` Entity, @generated by sea-orm-codegen 1.1.0

use anyhow::Result;
use ethportal_api::types::enr::Enr;
use ethportal_api::types::query_trace::QueryFailureKind;
use sea_orm::entity::prelude::*;

use super::record;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "audit_internal_failure")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub audit: i32,
    pub sender_node: i32,
    pub sender_record_id: i32,
    pub failure_type: TransferFailureType,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::content_audit::Entity",
        from = "Column::Audit",
        to = "super::content_audit::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ContentAudit,
    #[sea_orm(
        belongs_to = "super::node::Entity",
        from = "Column::SenderNode",
        to = "super::node::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Node,
}

impl Related<super::content_audit::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentAudit.def()
    }
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// *** Custom additions ***

#[derive(Debug, Clone, Eq, PartialEq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum TransferFailureType {
    InvalidContent = 0,
    UtpConnectionFailed = 1,
    UtpTransferFailed = 2,
}

impl From<QueryFailureKind> for TransferFailureType {
    fn from(kind: QueryFailureKind) -> Self {
        match kind {
            QueryFailureKind::InvalidContent => TransferFailureType::InvalidContent,
            QueryFailureKind::UtpConnectionFailed => TransferFailureType::UtpConnectionFailed,
            QueryFailureKind::UtpTransferFailed => TransferFailureType::UtpTransferFailed,
        }
    }
}

pub async fn create(
    audit_id: i32,
    sender_enr: &Enr,
    fail_type: TransferFailureType,
    conn: &DatabaseConnection,
) -> Result<Model> {
    let sender_record = record::get_or_create(sender_enr, conn).await?;

    let internal_failure = ActiveModel {
        audit: sea_orm::Set(audit_id),
        sender_node: sea_orm::Set(sender_record.node_id),
        sender_record_id: sea_orm::Set(sender_record.id),
        failure_type: sea_orm::Set(fail_type),
        ..Default::default()
    };

    Ok(internal_failure.insert(conn).await?)
}
