use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use entity::{client_info, content, content_audit, execution_metadata, key_value, node, record};

use crate::routes::{ClientDiversityResult, PaginatedCensusListResult, RawEnr};
use glados_core::stats::AuditStats;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub stats: [AuditStats; 3],
}

#[derive(Template)]
#[template(path = "paginated_census_list.html")]
pub struct PaginatedCensusListTemplate {
    pub census_data: Vec<PaginatedCensusListResult>,
    pub list_census_page_id: i32,
    pub max_census_id: i32,
}

#[derive(Template)]
#[template(path = "single_census_view.html")]
pub struct SingleCensusViewTemplate {
    pub client_diversity_data: Vec<ClientDiversityResult>,
    pub enr_list: Vec<RawEnr>,
    pub census_id: i32,
    pub max_census_id: i32,
    pub node_count: i32,
    pub created_at: String,
}

#[derive(Template)]
#[template(path = "census_explorer.html")]
pub struct CensusExplorerTemplate {}

#[derive(Template)]
#[template(path = "network_dashboard.html")]
pub struct NetworkDashboardTemplate {
    pub total_node_count: u64,
    pub total_enr_count: u64,
    pub recent_node_list: Vec<node::Model>,
    pub recent_enr_list: Vec<(record::Model, node::Model)>,
}

#[derive(Template)]
#[template(path = "node_detail.html")]
pub struct NodeDetailTemplate {
    pub node: node::Model,
    pub latest_enr: Option<record::Model>,
    pub latest_enr_key_value_list: Option<Vec<key_value::Model>>,
    pub enr_list: Vec<record::Model>,
    pub closest_node_list: Vec<node::ModelWithDistance>,
}

#[derive(Template)]
#[template(path = "enr_detail.html")]
pub struct EnrDetailTemplate {
    pub node: node::Model,
    pub enr: record::Model,
    pub key_value_list: Vec<key_value::Model>,
}

pub type AuditTuple = (content_audit::Model, content::Model, client_info::Model);

#[derive(Template)]
#[template(path = "content_dashboard.html")]
pub struct ContentDashboardTemplate {
    pub stats: [AuditStats; 3],
    pub contentid_list: Vec<content::Model>,
    pub audits_of_recent_content: Vec<AuditTuple>,
    pub recent_audits: Vec<AuditTuple>,
    pub recent_audit_successes: Vec<AuditTuple>,
    pub recent_audit_failures: Vec<AuditTuple>,
}

#[derive(Template)]
#[template(path = "contentid_list.html")]
pub struct ContentIdListTemplate {
    pub contentid_list: Vec<content::Model>,
}

#[derive(Template)]
#[template(path = "contentid_detail.html")]
pub struct ContentIdDetailTemplate {
    pub content_id: content::Model,
    pub contentkey_list: Vec<content::Model>,
}

#[derive(Template)]
#[template(path = "contentaudit_detail.html")]
pub struct ContentAuditDetailTemplate {
    pub audit: content_audit::Model,
    pub content: content::Model,
    pub execution_metadata: execution_metadata::Model,
}

#[derive(Template)]
#[template(path = "contentkey_list.html")]
pub struct ContentKeyListTemplate {
    pub contentkey_list: Vec<content::Model>,
}

#[derive(Template)]
#[template(path = "audit_dashboard.html")]
pub struct AuditDashboardTemplate {}

#[derive(Template)]
#[template(path = "audit_table.html")]
pub struct AuditTableTemplate {
    pub stats: [AuditStats; 3],
    pub audits: Vec<AuditTuple>,
}

#[derive(Template)]
#[template(path = "contentkey_detail.html")]
pub struct ContentKeyDetailTemplate {
    pub content_key_model: content::Model,
    pub content_key: String,
    pub content_id: String,
    pub content_kind: String,
    pub block_number: Option<i32>,
    pub contentaudit_list: Vec<content_audit::Model>,
}

pub struct HtmlTemplate<T: Template>(pub T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}
