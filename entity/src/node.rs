//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.7
use anyhow::Result;
use enr::NodeId;

use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "node")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
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

pub async fn get_or_create(node_id: NodeId, conn: &DatabaseConnection) -> Result<Model> {
    // First try to lookup an existing entry.
    if let Some(node_id_model) = Entity::find()
        .filter(Column::NodeId.eq(node_id.raw().as_slice().to_vec()))
        .one(conn)
        .await?
    {
        // If there is an existing record, return it
        return Ok(node_id_model);
    }

    // If no record exists, create one and return it
    let node_id_model = ActiveModel {
        id: NotSet,
        node_id: Set(node_id.raw().into()),
    };
    Ok(node_id_model.insert(conn).await?)
}
