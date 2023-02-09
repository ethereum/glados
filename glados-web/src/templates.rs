use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

use entity::{contentaudit, contentkey, node};

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
    pub contentkey_list: Vec<contentkey::Model>,
    pub contentaudit_list: Vec<contentaudit::Model>,
}

#[derive(Template)]
#[template(path = "contentkey_list.html")]
pub struct ContentKeyListTemplate {
    pub contentkey_list: Vec<contentkey::Model>,
}

#[derive(Template)]
#[template(path = "contentkey_detail.html")]
pub struct ContentKeyDetailTemplate {
    pub content_key: contentkey::Model,
    pub contentaudit_list: Vec<contentaudit::Model>,
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
