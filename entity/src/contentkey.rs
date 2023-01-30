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

pub async fn get_or_create<'b, T: OverlayContentKey>(
    content_key_raw: &'b T,
    conn: &DatabaseConnection,
) -> Result<Model>
where
    Vec<u8>: From<&'b T>,
{
    let encoded: Vec<u8> = content_key_raw.into();
    // First try to lookup an existing entry.
    let content_key = Entity::find()
        .filter(Column::ContentKey.eq(encoded.to_owned()))
        .one(conn)
        .await?;

    if let Some(content_key) = content_key {
        // If there is an existing record, return it
        return Ok(content_key);
    }
    let content_id_raw = content_key_raw.content_id();
    let content_id_hash = H256::from_slice(&content_id_raw);
    let content_id = contentid::get_or_create(&content_id_hash, conn).await?;
    // If no record exists, create one and return it
    let content_key = ActiveModel {
        id: NotSet,
        content_id: Set(content_id.id),
        content_key: Set(encoded),
        created_at: Set(chrono::offset::Utc::now()),
    };
    Ok(content_key.insert(conn).await?)
}

pub async fn get<'b, T: OverlayContentKey>(
    content_key: &'b T,
    conn: &DatabaseConnection,
) -> Result<Option<Model>>
where
    Vec<u8>: From<&'b T>,
{
    let encoded: Vec<u8> = content_key.into();
    Ok(Entity::find()
        .filter(Column::ContentKey.eq(encoded))
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
