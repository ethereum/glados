use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use entity::{content, content_audit, key_value, node, record};

use crate::routes::Stats;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {}

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

#[derive(Template)]
#[template(path = "content_dashboard.html")]
pub struct ContentDashboardTemplate {
    pub stats: [Stats; 3],
    pub contentid_list: Vec<content::Model>,
    pub recent_content: Vec<(content::Model, content_audit::Model)>,
    pub recent_audits: Vec<(content::Model, content_audit::Model)>,
    pub recent_audit_successes: Vec<(content::Model, content_audit::Model)>,
    pub recent_audit_failures: Vec<(content::Model, content_audit::Model)>,
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
}

#[derive(Template)]
#[template(path = "contentkey_list.html")]
pub struct ContentKeyListTemplate {
    pub contentkey_list: Vec<content::Model>,
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
