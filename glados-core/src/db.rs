use entity::{content, Subprotocol};
use ethportal_api::{HistoryContentKey, OverlayContentKey};
use sea_orm::DatabaseConnection;
use tracing::error;

/// Stores the content key from the History sub protocol.
///
/// Errors are logged.
pub async fn store_history_content_key(
    key: &HistoryContentKey,
    block_number: u64,
    conn: &DatabaseConnection,
) -> Option<content::Model> {
    content::get_or_create(Subprotocol::History, key, Some(block_number), conn)
        .await
        .inspect_err(|err| {
            error!(
                content.key = key.to_hex(),
                content.type = ?key.as_ref(),
                ?err,
                "Failed to create new history content",
            )
        })
        .ok()
}
