use std::io;
use std::sync::Arc;

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect};

use entity::{contentaudit, contentkey, node};

use crate::state::State;
use crate::templates::{
    ContentDashboardTemplate, ContentKeyDetailTemplate, ContentKeyListTemplate, HtmlTemplate,
    IndexTemplate, NodeListTemplate,
};

//
// Routes
//
pub async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}

pub async fn root(Extension(_state): Extension<Arc<State>>) -> impl IntoResponse {
    let template = IndexTemplate {};
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

pub async fn content_dashboard(Extension(state): Extension<Arc<State>>) -> impl IntoResponse {
    let contentkey_list = contentkey::Entity::find()
        .order_by_desc(contentkey::Column::ContentKey)
        .limit(10)
        .all(&state.database_connection)
        .await
        .unwrap();
    let contentaudit_list = contentaudit::Entity::find()
        .order_by_desc(contentaudit::Column::CreatedAt)
        .limit(10)
        .all(&state.database_connection)
        .await
        .unwrap();
    let template = ContentDashboardTemplate {
        contentkey_list,
        contentaudit_list,
    };
    HtmlTemplate(template)
}

pub async fn contentkey_list(Extension(state): Extension<Arc<State>>) -> impl IntoResponse {
    let contentkey_list: Vec<contentkey::Model> = contentkey::Entity::find()
        .order_by_desc(contentkey::Column::Id)
        .limit(50)
        .all(&state.database_connection)
        .await
        .unwrap();
    let template = ContentKeyListTemplate { contentkey_list };
    HtmlTemplate(template)
}

pub async fn contentkey_detail(
    Path(content_key_hex): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    let content_key_raw = hex::decode(&content_key_hex[2..]).unwrap();
    let content_key = contentkey::Entity::find()
        .filter(contentkey::Column::ContentKey.eq(content_key_raw))
        .one(&state.database_connection)
        .await
        .unwrap()
        .expect("No content found");

    let contentaudit_list = content_key
        .find_related(contentaudit::Entity)
        .all(&state.database_connection)
        .await
        .unwrap();

    let template = ContentKeyDetailTemplate {
        content_key,
        contentaudit_list,
    };
    HtmlTemplate(template)
}
