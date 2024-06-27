use std::{collections::HashMap, hash::Hash};

use super::query_response_node;
use anyhow::Result;
use enr::NodeId;
use ethportal_api::types::query_trace::QueryResponse;
use sea_orm::{entity::prelude::*, ActiveValue::NotSet, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "query_response")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub query_trace_id: i32,
    pub node_id: i32,
    pub duration_ms: u32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::query_trace::Entity",
        from = "Column::QueryTraceId",
        to = "super::query_trace::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    QueryTrace,
    #[sea_orm(has_many = "super::query_response_node::Entity")]
    QueryResponseNode,
}

impl Related<super::query_trace::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::QueryTrace.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Create a new query response entity.
pub async fn create(
    query_trace_id: i32,
    query_responses: HashMap<NodeId, QueryResponse>,
    conn: &DatabaseConnection,
) -> Result<Model> {
    // Get the node ID for the node that responded.
    let node = match super::node::Entity::find()
        .filter(super::node::Column::NodeId.eq(node_id.raw().to_vec()))
        .one(conn)
        .await?
    {
        Some(node) => node,
        None => {
            return Err(anyhow::anyhow!("No node found for node ID {}", node_id));
        }
    };

    let duration_ms = query_response.duration_ms as u32;

    let query_response_model = ActiveModel {
        id: NotSet,
        query_trace_id: Set(query_trace_id),
        node_id: Set(node.id),
        duration_ms: Set(duration_ms),
    };
    let query_response_model = query_response_model.insert(conn).await?;

    // Lookup node IDs of all nodes that responded.

    // For each node that responded, look up the node and create a new query response node entity.
    query_response_node::create(query_response_model.id, query_response, conn).await?;

    Ok(query_response_model)
}
