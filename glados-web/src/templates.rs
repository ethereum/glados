use askama::Template;
use axum::{
    response::{Html, IntoResponse, Response},
    http::StatusCode,
};

use glados_core::jsonrpc::{RoutingTableInfo, NodeInfo};


#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub ipc_path: String,
    pub client_version: String,
    pub node_info: NodeInfo,
    pub routing_table_info: RoutingTableInfo,
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
