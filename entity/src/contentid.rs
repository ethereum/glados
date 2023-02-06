use anyhow::{Context, Result};
use ethereum_types::H256;
use sea_orm::entity::prelude::*;
use sea_orm::{NotSet, Set};

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content_id")]
pub struct Model {
    #[sea_orm(primary_key, indexed)]
    pub id: i32,
    #[sea_orm(unique, indexed)]
    pub content_id: Vec<u8>,
}

impl Model {
    pub fn as_hash(&self) -> H256 {
        H256::from_slice(&self.content_id)
    }

    pub fn as_hex(&self) -> String {
        format!("{:#x}", self.as_hash())
    }
}

pub async fn get_or_create(content_id_hash: &H256, conn: &DatabaseConnection) -> Result<Model> {
    // First try to lookup an existing entry.
    let content_id = Entity::find()
        .filter(Column::ContentId.eq(content_id_hash.as_bytes()))
        .one(conn)
        .await?;

    if let Some(content_id) = content_id {
        // If there is an existing record, return it
        Ok(content_id)
    } else {
        // If no record exists, create one and return it
        let content_id = ActiveModel {
            id: NotSet,
            content_id: Set(content_id_hash.as_bytes().to_vec()),
        };
        content_id
            .insert(conn)
            .await
            .with_context(|| "Error inserting new content_id")
    }
}

impl Model {
    pub fn content_id_hash(&self) -> H256 {
        H256::from_slice(&self.content_id)
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::contentkey::Entity")]
    ContentKey,
}

impl Related<super::contentkey::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
