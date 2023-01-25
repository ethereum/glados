use sea_orm::entity::prelude::*;
use sea_orm::{NotSet, Set};

use chrono::{DateTime, Utc};

use glados_core::types::ContentKey;

use crate::contentid;

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content_key")]
pub struct Model {
    #[sea_orm(primary_key, indexed)]
    pub id: i32,
    #[sea_orm(unique, indexed)]
    pub content_id: i32,
    #[sea_orm(unique, indexed)]
    pub content_key: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

impl Model {
    pub fn as_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.content_key))
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contentid::Entity",
        from = "Column::ContentId",
        to = "super::contentid::Column::Id"
    )]
    ContentId,
    #[sea_orm(has_many = "super::contentaudit::Entity")]
    ContentAudit,
}

pub async fn get_or_create(
    content_key_raw: &impl ContentKey,
    conn: &DatabaseConnection,
) -> Result<Model, DbErr> {
    // First try to lookup an existing entry.
    let content_key = Entity::find()
        .filter(Column::ContentKey.eq(content_key_raw.encode()))
        .one(conn)
        .await?;

    if let Some(content_key) = content_key {
        // If there is an existing record, return it
        return Ok(content_key);
    }
    let content_id_raw = content_key_raw.content_id();
    let content_id = contentid::get_or_create(&content_id_raw, conn).await;
    // If no record exists, create one and return it
    let content_key = ActiveModel {
        id: NotSet,
        content_id: Set(content_id.id),
        content_key: Set(content_key_raw.encode()),
        created_at: Set(chrono::offset::Utc::now()),
    };
    content_key.insert(conn).await
}

pub async fn get(content_key: &impl ContentKey, conn: &DatabaseConnection) -> Option<Model> {
    Entity::find()
        .filter(Column::ContentKey.eq(content_key.encode()))
        .one(conn)
        .await
        .unwrap()
}

impl Related<super::contentid::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentId.def()
    }
}

impl Related<super::contentaudit::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentAudit.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
