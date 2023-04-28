use anyhow::{anyhow, Context, Result};
use ethportal_api::{ContentValue, HistoryContentKey, HistoryContentValue, OverlayContentKey};
use thiserror::Error;
use tracing::warn;
use trin_utils::bytes::hex_encode;
use web3::{
    transports::Http,
    types::{Block, BlockId, H256},
    Web3,
};

/// A connection to a trusted Ethereum execution node.
#[derive(Clone, Debug)]
pub enum Provider {
    Http(Web3<Http>),
    Pandaops(Web3<Http>),
}

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("unable to perform validation {0}")]
    Infeasible(anyhow::Error),
    #[error("content different to expected {0}")]
    Invalid(#[from] anyhow::Error),
}

impl Provider {
    pub async fn get_block_header(&self, block_hash: &[u8; 32]) -> Result<Option<Block<H256>>> {
        match self {
            Provider::Http(web3) => {
                let block_hash = BlockId::Hash(block_hash.into());
                Ok(web3.eth().block(block_hash).await?)
            }
            Provider::Pandaops(web3) => {
                let block_hash = BlockId::Hash(block_hash.into());
                Ok(web3.eth().block(block_hash).await?)
            }
        }
    }
}

/// Checks that content bytes correspond to a correctly formatted
/// content value.
///
/// Errors are logged.
pub async fn content_is_valid(
    provider: &Provider,
    content_key: &HistoryContentKey,
    content_bytes: &[u8],
) -> Result<bool, ValidationError> {
    // check deserialization is valid
    let content: HistoryContentValue = match HistoryContentValue::decode(content_bytes) {
        Ok(c) => c,
        Err(e) => {
            warn!(content.value=hex_encode(content_bytes), err=?e, "could not deserialize content bytes");
            return Ok(false);
        }
    };

    // check nature of content is valid
    match check_content_correctness(content_key, content, provider).await {
        Ok(_) => Ok(true),
        Err(ValidationError::Invalid(e)) => {
            warn!(content.value=hex_encode(content_bytes), err=?e, "content from portal node
            is different to content from trusted provider");
            Ok(false)
        }
        Err(ValidationError::Infeasible(e)) => Err(ValidationError::Infeasible(e)),
    }
}

/// Returns an error if the content is different from that received from a trusted provider.
async fn check_content_correctness(
    content_key: &HistoryContentKey,
    content: HistoryContentValue,
    provider: &Provider,
) -> Result<(), ValidationError> {
    match content {
        HistoryContentValue::BlockHeaderWithProof(h) => {
            // Reconstruct the key using the block header contents (RLP then hash).
            let computed = h.header.hash().to_fixed_bytes();
            let trusted = block_hash_from_key(content_key)?;
            if computed != trusted {
                return Err(ValidationError::Invalid(anyhow!(
                    "computed header hash {} did not match expected {}",
                    hex_encode(computed),
                    hex_encode(trusted)
                )));
            }
        }
        HistoryContentValue::BlockBody(b) => {
            // Reconstruct the key using the block body contents.
            let computed_tx_root = b.transactions_root()?;
            let computed_uncles_root = b.uncles_root()?;
            let block_hash = block_hash_from_key(content_key)?;
            let header = fetch_block_header(&block_hash, provider).await?;

            if header.transactions_root != computed_tx_root {
                return Err(ValidationError::Invalid(anyhow!(
                    "computed transactions root {} different from trusted provider {}",
                    hex_encode(computed_tx_root),
                    hex_encode(header.transactions_root)
                )));
            };
            if header.uncles_hash != computed_uncles_root {
                return Err(ValidationError::Invalid(anyhow!(
                    "computed uncles root {} different from trusted provider {}",
                    hex_encode(computed_uncles_root),
                    hex_encode(header.uncles_hash)
                )));
            };
        }
        HistoryContentValue::Receipts(r) => {
            // Reconstruct the key using the block body contents.
            let computed_receipts_root = r.root()?;
            let block_hash = block_hash_from_key(content_key)?;
            let header = fetch_block_header(&block_hash, provider).await?;
            if header.receipts_root != computed_receipts_root {
                return Err(ValidationError::Invalid(anyhow!(
                    "computed receipts root {} different from trusted provider {}",
                    hex_encode(computed_receipts_root),
                    hex_encode(header.receipts_root)
                )));
            }
        }
        HistoryContentValue::EpochAccumulator(_e) => {
            warn!("epoch master accumulator structural check passed, but correctness check unimplemented.")
        }
    }
    Ok(())
}

/// Calls trusted provider to get block header for given content key.
async fn fetch_block_header(
    block_hash: &[u8; 32],
    provider: &Provider,
) -> Result<Block<H256>, ValidationError> {
    let header = provider
        .get_block_header(block_hash)
        .await
        .with_context(|| "unable to retrieve block header from trusted provider")
        .map_err(ValidationError::Infeasible)?;
    header
        .ok_or_else(|| anyhow!("no block header available from trusted provider for validation"))
        .map_err(ValidationError::Infeasible)
}

// Removes the selector from the content key bytes to obtain the block hash.
fn block_hash_from_key(content_key: &HistoryContentKey) -> anyhow::Result<[u8; 32]> {
    let key_bytes = content_key.to_bytes();
    let (_selector, block_hash_slice) = key_bytes.split_at(1);
    block_hash_slice
        .try_into()
        .with_context(|| "unable to derive block hash from content key")
}
