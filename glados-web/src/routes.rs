use std::sync::Arc;

use axum::{extract::Extension, response::IntoResponse};
use sea_orm::{ActiveModelTrait, EntityTrait, NotSet, QueryOrder, QuerySelect, Set};

use glados_core::jsonrpc::PortalClient;

use entity::node;

use crate::state::State;
use crate::templates::{HtmlTemplate, IndexTemplate, NodeListTemplate};

//
// Routes
//
pub async fn root(Extension(state): Extension<Arc<State>>) -> impl IntoResponse {
    let ipc_path = state
        .ipc_path
        .as_os_str()
        .to_os_string()
        .into_string()
        .unwrap();
    let mut client = PortalClient::from_ipc(&state.ipc_path).unwrap();

    let client_version = client.get_client_version();
    let node_info = client.get_node_info();
    let routing_table_info = client.get_routing_table_info();

    let node = node::ActiveModel {
        id: NotSet,
        node_id: Set(node_info.nodeId.as_bytes().to_vec()),
    };
    match node.insert(&state.database_connection).await {
        Ok(_result) => println!("db success"),
        Err(err) => println!("db error: {}", err),
    }

    let template = IndexTemplate {
        ipc_path,
        client_version,
        node_info,
        routing_table_info,
    };
    HtmlTemplate(template)
}

pub async fn node_list(Extension(state): Extension<Arc<State>>) -> impl IntoResponse {
    let nodes: Vec<node::Model> = node::Entity::find()
        .order_by_asc(node::Column::NodeId)
        .limit(50)
        .all(&state.database_connection)
        .await
        .unwrap();
    let template = NodeListTemplate { nodes };
    HtmlTemplate(template)
}
