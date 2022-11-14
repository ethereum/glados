use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "content_key")]
pub struct Model {
    #[sea_orm(primary_key, indexed)]
    pub id: i32,
    #[sea_orm(unique, indexed)]
    pub content_id: i32,
    #[sea_orm(unique, indexed)]
    pub content_key: Vec<u8>,
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
