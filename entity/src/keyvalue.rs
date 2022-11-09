use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "keyvalue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub record_id: i32,
    pub key: Vec<u8>,
    pub value: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Record,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Record => Entity::belongs_to(super::record::Entity)
                .from(Column::RecordId)
                .to(super::record::Column::Id)
                .into(),
        }
    }
}

impl Related<super::record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Record.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
