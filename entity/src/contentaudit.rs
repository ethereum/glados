use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

use glados_core::types::ContentKey;

use std::i32;

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

impl Model {
    pub fn is_success(&self) -> bool {
        self.result == AuditResult::Success
    }
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

pub async fn create(
    content_key_id: i32,
    query_successful: bool,
    conn: &DatabaseConnection,
) -> Model {
    // If no record exists, create one and return it
    let audit_result = if query_successful {
        AuditResult::Success
    } else {
        AuditResult::Failure
    };

    let content_audit = ActiveModel {
        id: NotSet,
        content_key: Set(content_key_id),
        created_at: Set(chrono::offset::Utc::now()),
        result: Set(audit_result),
    };
    content_audit
        .insert(conn)
        .await
        .expect("Error inserting new content_audit")
}

pub async fn get_audits(content_key: &impl ContentKey, conn: &DatabaseConnection) -> Vec<Model> {
    let audits = Entity::find()
        .filter(Column::ContentKey.eq(content_key.encode()))
        .all(conn)
        .await
        .unwrap();
    audits
}

impl Related<super::contentkey::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
