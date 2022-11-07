use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "keyvalue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub enr_id: i32,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Enr,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Enr => Entity::belongs_to(super::enr::Entity)
                .from(Column::EnrId)
                .to(super::enr::Column::Id)
                .into(),
        }
    }
}

impl Related<super::enr::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Enr.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
