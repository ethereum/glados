use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use entity::{content, content_audit, node};

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {}

#[derive(Template)]
#[template(path = "node_list.html")]
pub struct NodeListTemplate {
    pub nodes: Vec<node::Model>,
}

#[derive(Template)]
#[template(path = "content_dashboard.html")]
pub struct ContentDashboardTemplate {
    pub contentid_list: Vec<content::Model>,
    pub contentaudit_list: Vec<content_audit::Model>,
    pub contentaudit_pass_list: Vec<content_audit::Model>,
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
                format!("Failed to render template. Error: {err}"),
            )
                .into_response(),
        }
    }
}
