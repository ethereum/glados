use sea_orm::entity::prelude::*;
use sea_orm::{NotSet, Set};

use chrono::{DateTime, Utc};

use glados_core::types::ContentKey;

use crate::contentid;

#[derive(Clone, Debug, Eq, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "execution_header")]
pub struct Model {
    #[sea_orm(primary_key, indexed)]
    pub id: i32,
    #[sea_orm(unique, indexed)]
    pub contentid_id: i32,
    #[sea_orm(unique, indexed)]
    pub block_number: i32,
    #[sea_orm(unique, indexed)]
    pub block_hash: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::contentid::Entity",
        from = "Column::ContentidId",
        to = "super::contentid::Column::Id"
    )]
    ContentidId,
}

impl Related<super::contentid::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentidId.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
