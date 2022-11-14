use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "node")]
pub struct Model {
    #[sea_orm(primary_key, indexed)]
    pub id: i32,
    #[sea_orm(unique, indexed)]
    pub node_id: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::record::Entity")]
    Record,
}

impl Related<super::record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Record.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
