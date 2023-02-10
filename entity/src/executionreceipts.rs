//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.7

use anyhow::Result;
use ethportal_api::types::content_key::OverlayContentKey;
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

use crate::contentkey;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "execution_receipts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(unique)]
    pub content_key: i32,
    #[sea_orm(unique)]
    pub block_number: i32,
    #[sea_orm(unique)]
    pub block_hash: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contentkey::Entity",
        from = "Column::ContentKey",
        to = "super::contentkey::Column::Id",
        on_update = "Cascade",
        on_delete = "SetNull"
    )]
    ContentKey,
}

impl Related<super::contentkey::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentKey.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub async fn get_or_create<T: OverlayContentKey>(
    content_key: &T,
    block_number: i32,
    block_hash: &[u8; 32],
    conn: &DatabaseConnection,
) -> Result<Model> {
    // TODO (Perama 2023-01-28) replace .clone() with ".bytes()" here (see cdc09d2).
    let hash = block_hash.clone().to_vec();
    // First try to lookup an existing entry.
    let receipts = Entity::find()
        .filter(Column::BlockHash.eq(hash.clone()))
        .one(conn)
        .await?;

    if let Some(receipts) = receipts {
        // If there is an existing record, return it
        return Ok(receipts);
    }
    // If no record exists, create one and return it
    let content_key_model = contentkey::get_or_create(content_key, conn).await?;

    let receipts_model = ActiveModel {
        id: NotSet,
        content_key: Set(content_key_model.id),
        block_number: Set(block_number),
        block_hash: Set(hash),
    };
    Ok(receipts_model.insert(conn).await?)
}

/// Returns the an execution receipts monitor item if present.
///
/// Used during auditing. Returning none indicates a failed audit.
///
/// The foreign id is the database-assigned id for a content_key
/// (not the portal network content_id).
pub async fn get(
    // The database-assigned id for a content key.
    //
    // This is not the portal network content_id.
    content_key_foreign_id: i32,
    conn: &DatabaseConnection,
) -> Result<Option<Model>, DbErr> {
    Entity::find()
        .filter(Column::ContentKey.eq(content_key_foreign_id))
        .one(conn)
        .await
}
