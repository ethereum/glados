use anyhow::Result;
use chrono::{DateTime, Utc};
use ethereum_types::H256;
use ethportal_api::types::content_key::OverlayContentKey;
use sea_orm::entity::prelude::*;
use sea_orm::{NotSet, Set};

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

pub async fn get_or_create<T: OverlayContentKey>(
    content_key: &T,
    conn: &DatabaseConnection,
) -> Result<Model> {
    // The passing of &OverlayContentKey is currently limited (requires lifetimes).
    // This is a temporary fix / reminder. Likely solutions are that OverlayContentKey should:
    // 1. Have a method `fn bytes(&self) -> [u8; u32]` that can replace `clone()` here.
    // 2. Be `From<&Self>`, which when impl should internall call self.clone().
    let content_key_bytes: Vec<u8> = content_key.clone().into();
    // First try to lookup an existing entry.
    if let Some(content_key_model) = Entity::find()
        .filter(Column::ContentKey.eq(content_key_bytes.clone()))
        .one(conn)
        .await?
    {
        // If there is an existing record, return it
        return Ok(content_key_model);
    }

    let content_id = content_key.content_id();
    let content_id_hash = H256::from_slice(&content_id);
    let content_id_model = contentid::get_or_create(&content_id_hash, conn).await?;
    // If no record exists, create one and return it
    let content_key = ActiveModel {
        id: NotSet,
        content_id: Set(content_id_model.id),
        content_key: Set(content_key_bytes),
        created_at: Set(chrono::offset::Utc::now()),
    };
    Ok(content_key.insert(conn).await?)
}

pub async fn get<T: OverlayContentKey>(
    content_key: &T,
    conn: &DatabaseConnection,
) -> Result<Option<Model>> {
    let content_key_bytes: Vec<u8> = content_key.clone().into();
    Ok(Entity::find()
        .filter(Column::ContentKey.eq(content_key_bytes))
        .one(conn)
        .await?)
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
