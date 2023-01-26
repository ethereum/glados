pub(crate) use std::fmt;

use ethereum_types::H256;
use sha2::{Digest, Sha256};

pub enum ContentKeySelector {
    BlockHeader,
    BlockBody,
    BlockReceipts,
    EpochAccumulator,
}

impl ContentKeySelector {
    fn value(&self) -> u8 {
        match *self {
            ContentKeySelector::BlockHeader => 0,
            ContentKeySelector::BlockBody => 1,
            ContentKeySelector::BlockReceipts => 2,
            ContentKeySelector::EpochAccumulator => 3,
        }
    }
}

pub trait ContentKey: fmt::Debug {
    const SELECTOR: ContentKeySelector;

    fn encode(&self) -> Vec<u8>;

    fn hex_encode(&self) -> String {
        format!("0x{}", hex::encode(self.encode()))
    }

    fn content_id(&self) -> H256 {
        let mut hasher = Sha256::new();
        hasher.update(self.encode());
        let raw_hash = hasher.finalize();
        H256::from_slice(&raw_hash)
    }
}

#[derive(Debug)]
pub struct BlockHeaderContentKey {
    pub hash: H256,
}

impl fmt::Display for BlockHeaderContentKey {
    // TODO: how can this be implemented generically
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(key={}, cid={})", self.hex_encode(), self.content_id())
    }
}

impl ContentKey for BlockHeaderContentKey {
    const SELECTOR: ContentKeySelector = ContentKeySelector::BlockHeader;

    fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = vec![BlockHeaderContentKey::SELECTOR.value()];
        encoded.extend_from_slice(&self.hash[..]);
        encoded
    }
}

#[derive(Debug)]
pub struct BlockBodyContentKey {
    pub hash: H256,
}

impl fmt::Display for BlockBodyContentKey {
    // TODO: how can this be implemented generically
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(key={}, cid={})", self.hex_encode(), self.content_id())
    }
}

impl ContentKey for BlockBodyContentKey {
    const SELECTOR: ContentKeySelector = ContentKeySelector::BlockBody;

    fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = vec![BlockBodyContentKey::SELECTOR.value()];
        encoded.extend_from_slice(&self.hash[..]);
        encoded
    }
}

#[derive(Debug)]
pub struct BlockReceiptsContentKey {
    pub hash: H256,
}

impl fmt::Display for BlockReceiptsContentKey {
    // TODO: how can this be implemented generically
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(key={}, cid={})", self.hex_encode(), self.content_id())
    }
}

impl ContentKey for BlockReceiptsContentKey {
    const SELECTOR: ContentKeySelector = ContentKeySelector::BlockReceipts;

    fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = vec![BlockReceiptsContentKey::SELECTOR.value()];
        encoded.extend_from_slice(&self.hash[..]);
        encoded
    }
}

#[derive(Debug)]
pub struct EpochAccumulatorContentKey {
    pub hash: H256,
}

impl fmt::Display for EpochAccumulatorContentKey {
    // TODO: how can this be implemented generically
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(key={}, cid={})", self.hex_encode(), self.content_id())
    }
}

impl ContentKey for EpochAccumulatorContentKey {
    const SELECTOR: ContentKeySelector = ContentKeySelector::EpochAccumulator;

    fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = vec![EpochAccumulatorContentKey::SELECTOR.value()];
        encoded.extend_from_slice(&self.hash[..]);
        encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hex;

    #[test]
    fn test_block_header_encode() {
        let raw_hash =
            hex::decode("d1c390624d3bd4e409a61a858e5dcc5517729a9170d014a6c96530d64dd8621d")
                .unwrap();
        let block_hash = H256::from_slice(&raw_hash);

        let content_key = BlockHeaderContentKey { hash: block_hash };
        assert_eq!(content_key.hash.as_bytes(), raw_hash);
    }

    #[test]
    fn test_block_header_hex_encode() {
        let raw_hash =
            hex::decode("d1c390624d3bd4e409a61a858e5dcc5517729a9170d014a6c96530d64dd8621d")
                .unwrap();
        let block_hash = H256::from_slice(&raw_hash);

        let content_key = BlockHeaderContentKey { hash: block_hash };
        assert_eq!(
            content_key.hex_encode(),
            "0x00d1c390624d3bd4e409a61a858e5dcc5517729a9170d014a6c96530d64dd8621d"
        );
    }

    #[test]
    fn test_block_header_content_key() {
        let raw_hash =
            hex::decode("d1c390624d3bd4e409a61a858e5dcc5517729a9170d014a6c96530d64dd8621d")
                .unwrap();
        let block_hash = H256::from_slice(&raw_hash);
        let content_key = BlockHeaderContentKey { hash: block_hash };

        assert_eq!(
            content_key.content_id().as_bytes(),
            hex::decode("3e86b3767b57402ea72e369ae0496ce47cc15be685bec3b4726b9f316e3895fe")
                .unwrap()
        );
    }
}
