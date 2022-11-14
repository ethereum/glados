use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "record")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub node_id: Vec<u8>,
    pub sequence_number: i32,
    pub raw: Vec<u8>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {
    Node,
}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        match self {
            Self::Node => Entity::belongs_to(super::node::Entity)
                .from(Column::NodeId)
                .to(super::node::Column::NodeId)
                .into(),
        }
    }
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
