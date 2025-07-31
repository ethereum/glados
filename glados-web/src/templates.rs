use std::collections::HashMap;

use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use enr::NodeId;
use entity::{
    audit, client,
    client_info::{Client, OperatingSystem},
    content, node, node_enr, ContentType, Subprotocol,
};
use glados_core::stats::{AuditStats, StrategyFilter};
use strum::EnumProperty;

use crate::routes::{
    CalculatedRadiusChartData, ClientDiversityResult, NodeEnr, PaginatedCensusListResult,
    TransferFailure,
};

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub subprotocol: Subprotocol,
    pub strategy: StrategyFilter,
    pub client_diversity_data: Vec<ClientDiversityResult>,
    pub average_radius_chart: Vec<CalculatedRadiusChartData>,
    pub stats: [AuditStats; 3],
    pub content_types: Vec<ContentType>,
    pub clients: Vec<Client>,
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
    pub enr_list: Vec<NodeEnr>,
    pub census_id: i32,
    pub max_census_id: i32,
    pub node_count: i32,
    pub created_at: String,
}

#[derive(Template)]
#[template(path = "census_explorer.html")]
pub struct CensusExplorerTemplate {}

#[derive(Template)]
#[template(path = "node_detail.html")]
pub struct NodeDetailTemplate {
    pub node: node::Model,
    pub latest_enr: Option<node_enr::Model>,
    pub enr_list: Vec<node_enr::Model>,
    pub closest_node_list: Vec<node::ModelWithDistance>,
}

#[derive(Template)]
#[template(path = "enr_detail.html")]
pub struct EnrDetailTemplate {
    pub node: node::Model,
    pub enr: node_enr::Model,
}

pub type AuditTuple = (audit::Model, content::Model, client::Model);

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
    pub audit: audit::Model,
    pub content: content::Model,
    pub node_details: HashMap<NodeId, Client>,
}

#[derive(Template)]
#[template(path = "contentkey_list.html")]
pub struct ContentKeyListTemplate {
    pub contentkey_list: Vec<content::Model>,
}

#[derive(Template)]
#[template(path = "audit_dashboard.html")]
pub struct AuditDashboardTemplate {
    pub subprotocol: Subprotocol,
}

#[derive(Template)]
#[template(path = "audit_table.html")]
pub struct AuditTableTemplate {
    pub stats: [AuditStats; 3],
    pub audits: Vec<AuditTuple>,
}

#[derive(Template)]
#[template(path = "contentkey_detail.html")]
pub struct ContentKeyDetailTemplate {
    pub content: content::Model,
    pub content_kind: String,
    pub audit_list: Vec<audit::Model>,
}

#[derive(Template)]
#[template(path = "clients.html")]
pub struct ClientsTemplate {
    pub subprotocol: Subprotocol,
    pub clients: Vec<Client>,
    pub operating_systems: Vec<OperatingSystem>,
}

#[derive(Template)]
#[template(path = "diagnostics.html")]
pub struct DiagnosticsTemplate {
    pub failures: Vec<TransferFailure>,
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
