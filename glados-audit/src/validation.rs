use entity::content;
use ethportal_api::utils::bytes::hex_encode;
use ethportal_api::{BeaconContentKey, BlockHeaderKey, HistoryContentKey, OverlayContentKey};
use ethportal_api::{BeaconContentValue, ContentValue, HistoryContentValue};
use tracing::warn;

/// Checks that content bytes correspond to a correctly formatted
/// content value.
pub fn content_is_valid(content: &content::Model, content_bytes: &[u8]) -> bool {
    match content.protocol_id {
        content::SubProtocol::History => {
            let content_key = match HistoryContentKey::try_from(content.content_key.clone()) {
                Ok(key) => key,
                Err(err) => {
                    warn!(err=?err, content.content_key=?content.content_key, "Failed to decode history content key.");
                    return false;
                }
            };
            validate_history(&content_key, content_bytes)
        }
        content::SubProtocol::State => {
            warn!("State content validation not yet implemented.");
            true
        }
        content::SubProtocol::Beacon => {
            let content_key = match BeaconContentKey::try_from(content.content_key.clone()) {
                Ok(key) => key,
                Err(err) => {
                    warn!(err=?err, content.content_key=?content.content_key, "Failed to decode beacon content key.");
                    return false;
                }
            };
            validate_beacon(&content_key, content_bytes)
        }
    }
}

fn validate_beacon(content_key: &BeaconContentKey, content_bytes: &[u8]) -> bool {
    let content: BeaconContentValue = match BeaconContentValue::decode(content_bytes) {
        Ok(c) => c,
        Err(e) => {
            warn!(content.key=hex_encode(content_key.to_bytes()), err=?e, "could not deserialize beacon content bytes");
            return false;
        }
    };

    match content {
        BeaconContentValue::HistoricalSummariesWithProof(_) => {
            warn!("Need to call trusted provider to check historical summaries correctness.");
            true
        }
        BeaconContentValue::LightClientBootstrap(_) => {
            warn!("Need to call trusted provider to check light client bootstrap correctness.");
            true
        }
        BeaconContentValue::LightClientUpdatesByRange(_) => {
            warn!(
                "Need to call trusted provider to check light client updates by range correctness."
            );
            true
        }
        BeaconContentValue::LightClientOptimisticUpdate(_) => {
            warn!("Need to call trusted provider to check light client optimistic update correctness.");
            true
        }
        BeaconContentValue::LightClientFinalityUpdate(_) => {
            warn!(
                "Need to call trusted provider to check light client finality update correctness."
            );
            true
        }
    }
}

fn validate_history(content_key: &HistoryContentKey, content_bytes: &[u8]) -> bool {
    // check deserialization is valid
    let content: HistoryContentValue = match HistoryContentValue::decode(content_bytes) {
        Ok(c) => c,
        Err(e) => {
            warn!(content.key=hex_encode(content_key.to_bytes()), err=?e, "could not deserialize history content bytes");
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
