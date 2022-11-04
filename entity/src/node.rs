use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "node")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::enr::Entity")]
    ENR,
}

impl Related<super::enr::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ENR.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
