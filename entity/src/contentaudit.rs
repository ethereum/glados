use std::i32;

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

use ethportal_api::types::content_key::OverlayContentKey;

use crate::contentkey;

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
    #[sea_orm(indexed)]
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
    content_key_model_id: i32,
    query_successful: bool,
    conn: &DatabaseConnection,
) -> Result<Model> {
    // If no record exists, create one and return it
    let audit_result = if query_successful {
        AuditResult::Success
    } else {
        AuditResult::Failure
    };

    let content_audit = ActiveModel {
        id: NotSet,
        content_key: Set(content_key_model_id),
        created_at: Set(chrono::offset::Utc::now()),
        result: Set(audit_result),
    };
    Ok(content_audit.insert(conn).await?)
}

pub async fn get_audits<T: OverlayContentKey>(
    content_key: &T,
    conn: &DatabaseConnection,
) -> Result<Vec<Model>> {
    let Some(content_key_model) = contentkey::get(content_key, conn).await?
    else {
       bail!("Expected stored content_key found none.")
    };
    Ok(Entity::find()
        .filter(Column::ContentKey.eq(content_key_model.id))
        .all(conn)
        .await?)
}

impl Related<super::contentkey::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
