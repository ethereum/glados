use std::io;
use std::sync::Arc;

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
};
use ethportal_api::{types::content_key::OverlayContentKey, HistoryContentKey};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect};

use entity::{
    content,
    content_audit::{self, AuditResult},
    execution_metadata, node,
};

use crate::state::State;
use crate::templates::{
    ContentDashboardTemplate, ContentIdDetailTemplate, ContentIdListTemplate,
    ContentKeyDetailTemplate, ContentKeyListTemplate, HtmlTemplate, IndexTemplate,
    NodeListTemplate,
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
    let contentid_list = content::Entity::find()
        .order_by_desc(content::Column::FirstAvailableAt)
        .limit(20)
        .all(&state.database_connection)
        .await
        .unwrap();

    let recent_content_model: Vec<(content::Model, Vec<content_audit::Model>)> =
        content::Entity::find()
            .order_by_desc(content::Column::FirstAvailableAt)
            .find_with_related(content_audit::Entity)
            .filter(content_audit::Column::Result.is_not_null())
            .limit(20)
            .all(&state.database_connection)
            .await
            .unwrap();

    let recent_audits_model: Vec<(content_audit::Model, Vec<content::Model>)> =
        content_audit::Entity::find()
            .order_by_desc(content_audit::Column::CreatedAt)
            .find_with_related(content::Entity)
            .limit(20)
            .all(&state.database_connection)
            .await
            .unwrap();

    let recent_audit_success_model: Vec<(content_audit::Model, Vec<content::Model>)> =
        content_audit::Entity::find()
            .order_by_desc(content_audit::Column::CreatedAt)
            .find_with_related(content::Entity)
            .filter(content_audit::Column::Result.eq(AuditResult::Success))
            .limit(20)
            .all(&state.database_connection)
            .await
            .unwrap();

    let recent_audit_failure_model: Vec<(content_audit::Model, Vec<content::Model>)> =
        content_audit::Entity::find()
            .order_by_desc(content_audit::Column::CreatedAt)
            .find_with_related(content::Entity)
            .filter(content_audit::Column::Result.eq(AuditResult::Failure))
            .limit(20)
            .all(&state.database_connection)
            .await
            .unwrap();

    let template = ContentDashboardTemplate {
        contentid_list,
        recent_content: content_model_to_display(recent_content_model),
        recent_audits: audit_model_to_display(recent_audits_model),
        recent_audit_successes: audit_model_to_display(recent_audit_success_model),
        recent_audit_failures: audit_model_to_display(recent_audit_failure_model),
    };
    HtmlTemplate(template)
}

/// Summary of a model result (content with vector of audits).
fn content_model_to_display(
    content_model: Vec<(content::Model, Vec<content_audit::Model>)>,
) -> Vec<(content::Model, content_audit::Model)> {
    content_model
        .into_iter()
        .map(|content| {
            let content_data = content.0;
            let mut audits = content.1;
            audits.sort_by(|a, b| a.id.cmp(&b.id));
            let latest_audit = audits
                .into_iter()
                // Choose the latest audit.
                .rev()
                .next()
                // We know there will beat least one audit because we filtered nulls out after joining.
                .expect("Tables should be filtered to exclude Null values.");

            (content_data, latest_audit)
        })
        .collect()
}

/// Summary of a model result (audit with vector of content).
fn audit_model_to_display(
    content_model: Vec<(content_audit::Model, Vec<content::Model>)>,
) -> Vec<(content::Model, content_audit::Model)> {
    content_model
        .into_iter()
        .map(|content| {
            let audit = content.0;
            let content_data = content.1;
            let content_data: entity::content::Model = content_data
                .into_iter()
                .next()
                // We know there will be one content because audits only have one one content foreign key.
                .expect("Tables should be filtered to exclude Null values.");
            (content_data, audit)
        })
        .collect()
}

pub async fn contentid_list(Extension(state): Extension<Arc<State>>) -> impl IntoResponse {
    let contentid_list: Vec<content::Model> = content::Entity::find()
        .order_by_asc(content::Column::ContentId)
        .limit(50)
        .all(&state.database_connection)
        .await
        .unwrap();
    let template = ContentIdListTemplate { contentid_list };
    HtmlTemplate(template)
}

pub async fn contentid_detail(
    Path(content_id_hex): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    let content_id_raw = hex::decode(&content_id_hex[2..]).unwrap();
    let content_id = content::Entity::find()
        .filter(content::Column::ContentId.eq(content_id_raw.clone()))
        .one(&state.database_connection)
        .await
        .unwrap()
        .expect("No content found");

    let contentkey_list = content::Entity::find()
        .filter(content::Column::ContentId.eq(content_id_raw))
        .all(&state.database_connection)
        .await
        .unwrap();

    let template = ContentIdDetailTemplate {
        content_id,
        contentkey_list,
    };
    HtmlTemplate(template)
}

pub async fn contentkey_list(Extension(state): Extension<Arc<State>>) -> impl IntoResponse {
    let contentkey_list: Vec<content::Model> = content::Entity::find()
        .order_by_desc(content::Column::Id)
        .limit(50)
        .all(&state.database_connection)
        .await
        .unwrap();
    let template = ContentKeyListTemplate { contentkey_list };
    HtmlTemplate(template)
}

/// Retrieves key details to display.
///
/// At present this assumes it is a HistoryContentKey.
pub async fn contentkey_detail(
    Path(content_key_hex): Path<String>,
    Extension(state): Extension<Arc<State>>,
) -> impl IntoResponse {
    let content_key_raw = hex::decode(&content_key_hex[2..]).unwrap();
    let content_key_model = content::Entity::find()
        .filter(content::Column::ContentKey.eq(content_key_raw.clone()))
        .one(&state.database_connection)
        .await
        .unwrap()
        .expect("No content found");

    let contentaudit_list = content_key_model
        .find_related(content_audit::Entity)
        .all(&state.database_connection)
        .await
        .expect("Could not look up audits.");

    let content_key: HistoryContentKey = HistoryContentKey::try_from(content_key_raw.clone())
        .expect("Could not convert key bytes into OverlayContentKey.");
    let metadata_model = execution_metadata::Entity::find()
        .filter(execution_metadata::Column::Content.eq(content_key_model.id))
        .one(&state.database_connection)
        .await
        .expect("No content found");
    let block_number = metadata_model.map(|m| m.block_number);

    let content_id = format!("0x{}", hex::encode(content_key.content_id()));
    let content_kind = content_key.to_string();
    let template = ContentKeyDetailTemplate {
        content_key: content_key_hex,
        content_key_model,
        contentaudit_list,
        content_id,
        content_kind,
        block_number,
    };
    HtmlTemplate(template)
}
