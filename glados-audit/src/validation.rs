use ethportal_api::types::content_key::{BlockHeaderKey, HistoryContentKey, OverlayContentKey};
use ethportal_api::utils::bytes::hex_encode;
use tracing::warn;
use trin_types::content_value::{ContentValue, HistoryContentValue};

/// Checks that content bytes correspond to a correctly formatted
/// content value.
pub fn content_is_valid(content_key: &HistoryContentKey, content_bytes: &[u8]) -> bool {
    // check deserialization is valid
    let content: HistoryContentValue = match HistoryContentValue::decode(content_bytes) {
        Ok(c) => c,
        Err(e) => {
            warn!(content.value=hex_encode(content_bytes), err=?e, "could not deserialize content bytes");
            return false;
        }
    };

    // check nature of content is valid
    match content {
        HistoryContentValue::BlockHeaderWithProof(h) => {
            // Reconstruct the key using the block header contents (RLP then hash).
            let computed_hash = h.header.hash();
            let computed_key = HistoryContentKey::BlockHeaderWithProof(BlockHeaderKey {
                block_hash: computed_hash.into(),
            });
            match content_key == &computed_key {
                true => true,
                false => {
                    warn!(
                        content.key = hex_encode(content_key.to_bytes()),
                        content.value = hex_encode(content_bytes),
                        "computed header hash did not match expected"
                    );
                    false
                }
            }
        }
        HistoryContentValue::BlockBody(b) => {
            // Reconstruct the key using the block body contents.
            let _computed_tx_root = b.transactions_root();
            let _computed_uncles_root = b.uncles_root();
            warn!("Need to call trusted provider to check block body correctness.");
            true
        }
        HistoryContentValue::Receipts(r) => {
            // Reconstruct the key using the block body contents.
            let _computed_receipts_root = r.root();
            warn!("Need to call trusted provider to check receipts correctness.");
            true
        }
        HistoryContentValue::EpochAccumulator(_e) => {
            warn!("Need to check epoch master accumulator for correctness.");
            true
        }
    }
}
