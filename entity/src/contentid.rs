use sea_orm::entity::prelude::*;

use ethereum_types::H256;

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content_id")]
pub struct Model {
    #[sea_orm(primary_key, indexed)]
    pub id: i32,
    #[sea_orm(unique, indexed)]
    pub content_id: Vec<u8>,
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
