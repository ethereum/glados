//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.7
use anyhow::Result;
use chrono::{DateTime, Utc};
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

use crate::content::SubProtocol;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "census")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub started_at: DateTime<Utc>,
    pub duration: i32,
    pub sub_network: SubProtocol,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::census_node::Entity")]
    CensusNode,
}

impl Related<super::census_node::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CensusNode.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

pub async fn create(
    started_at: DateTime<Utc>,
    duration: u32,
    subnetwork: SubProtocol,
    conn: &DatabaseConnection,
) -> Result<Model> {
    // If no record exists, create one and return it
    let content_audit = ActiveModel {
        id: NotSet,
        started_at: Set(started_at),
        duration: Set(duration as i32),
        sub_network: Set(subnetwork),
    };

    Ok(content_audit.insert(conn).await?)
}
