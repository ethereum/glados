use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content_id")]
pub struct Model {
    #[sea_orm(primary_key, indexed)]
    pub id: i32,
    #[sea_orm(unique, indexed)]
    pub content_id: Vec<u8>,
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
