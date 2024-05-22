//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.7
use anyhow::Result;
use ethportal_api::types::enr::Enr;
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "record")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub node_id: i32,
    pub raw: String,
    pub sequence_number: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::key_value::Entity")]
    KeyValue,
    #[sea_orm(
        belongs_to = "super::node::Entity",
        from = "Column::NodeId",
        to = "super::node::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Node,
}

impl Related<super::key_value::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::KeyValue.def()
    }
}

impl Related<super::node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Node.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub async fn get_or_create(enr: &Enr, conn: &DatabaseConnection) -> Result<Model> {
    let node_id = super::node::get_or_create(enr.node_id(), conn).await?;

    // First try to lookup an existing entry.
    if let Some(enr_model) = Entity::find()
        .filter(Column::NodeId.eq(node_id.id))
        .filter(Column::SequenceNumber.eq(enr.seq()))
        .one(conn)
        .await?
    {
        // If there is an existing record, return it
        return Ok(enr_model);
    }

    // Wrap-around large sequence numbers
    // TODO: migrate DB schema to use BigInt
    let seq: i32 = match enr.seq().try_into() {
        Ok(seq) => seq,
        Err(_) => {
            if enr.seq() > i32::MAX as u64 {
                (enr.seq() % i32::MAX as u64) as i32
            } else {
                enr.seq() as i32
            }
        }
    };

    // If no record exists, create one and return it
    let enr_model_unsaved = ActiveModel {
        id: NotSet,
        node_id: Set(node_id.id),
        raw: Set(enr.to_base64()),
        sequence_number: Set(seq),
    };
    let enr_model = enr_model_unsaved.insert(conn).await?;

    for (enr_key, enr_value) in enr.iter() {
        super::key_value::get_or_create(enr_model.id, enr_key, &enr_value.to_vec(), conn).await?;
    }

    Ok(enr_model)
}
