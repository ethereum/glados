use std::fmt;

use sha2::{Digest, Sha256};

use ethereum_types::H256;

pub trait ContentKey {
    fn encode(&self) -> Vec<u8>;

    fn hex_encode(&self) -> String {
        hex::encode(self.encode())
    }

    fn content_id(&self) -> H256;
}

impl fmt::Display for dyn ContentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(key={}, cid={})", self.hex_encode(), self.content_id())
    }
}

pub struct BlockHeaderContentKey {
    pub hash: H256,
}

impl BlockHeaderContentKey {}

unsafe impl Send for BlockHeaderContentKey {}
unsafe impl Sync for BlockHeaderContentKey {}

impl ContentKey for BlockHeaderContentKey {
    fn encode(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = vec![0];
        encoded.extend_from_slice(&self.hash[..]);
        encoded
    }

    fn content_id(&self) -> H256 {
        let mut hasher = Sha256::new();
        hasher.update(self.encode());
        let raw_hash = hasher.finalize();
        H256::from_slice(&raw_hash)
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
            "00d1c390624d3bd4e409a61a858e5dcc5517729a9170d014a6c96530d64dd8621d"
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
