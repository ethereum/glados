use entity::content;
use ethportal_api::utils::bytes::hex_encode;
use ethportal_api::{ContentValue, HistoryContentValue};
use ethportal_api::{HistoryContentKey, OverlayContentKey};
use tracing::error;

/// Checks the validity of the content.
pub fn content_is_valid(content: &content::Model, content_bytes: &[u8]) -> bool {
    match content.protocol_id {
        content::SubProtocol::History => {
            let Ok(content_key) = HistoryContentKey::try_from_bytes(&content.content_key) else {
                error!(
                    content.content_key = ?hex_encode(&content.content_key),
                    "Failed to decode history content key.",
                );
                return false;
            };
            validate_history(&content_key, content_bytes)
        }
    }
}

/// Validates the content key/value pair
///
/// Currently we only validate that contetn value decodes correctly.
fn validate_history(content_key: &HistoryContentKey, content_bytes: &[u8]) -> bool {
    HistoryContentValue::decode(content_key, content_bytes).is_ok()
}
