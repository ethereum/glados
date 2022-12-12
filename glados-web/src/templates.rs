use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use glados_core::jsonrpc::{NodeInfo, RoutingTableInfo};

use entity::contentaudit;
use entity::contentid;
use entity::node;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub ipc_path: String,
    pub client_version: String,
    pub node_info: NodeInfo,
    pub routing_table_info: RoutingTableInfo,
}

#[derive(Template)]
#[template(path = "node_list.html")]
pub struct NodeListTemplate {
    pub nodes: Vec<node::Model>,
}

#[derive(Template)]
#[template(path = "content_dashboard.html")]
pub struct ContentDashboardTemplate {
    pub contentid_list: Vec<contentid::Model>,
    pub contentaudit_list: Vec<contentaudit::Model>,
}

#[derive(Template)]
#[template(path = "contentid_list.html")]
pub struct ContentIdListTemplate {
    pub contentid_list: Vec<contentid::Model>,
}

#[derive(Template)]
#[template(path = "contentid_detail.html")]
pub struct ContentIdDetailTemplate {
    pub content_id: contentid::Model,
}

pub struct HtmlTemplate<T>(pub T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}
