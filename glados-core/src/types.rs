use sha2::{Digest, Sha256};

use ethereum_types::H256;

pub struct BlockHeaderContentKey {
    pub hash: H256,
}

impl BlockHeaderContentKey {
    pub fn encoded(&self) -> Vec<u8> {
        let mut encoded: Vec<u8> = vec![0];
        encoded.extend_from_slice(&self.hash[..]);
        encoded
    }

    pub fn content_id(&self) -> H256 {
        let mut hasher = Sha256::new();
        hasher.update(self.encoded());
        let raw_hash = hasher.finalize();
        H256::from_slice(&raw_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hex;

    #[test]
    fn test_block_header_encoded() {
        let raw_hash =
            hex::decode("d1c390624d3bd4e409a61a858e5dcc5517729a9170d014a6c96530d64dd8621d")
                .unwrap();
        let block_hash = H256::from_slice(&raw_hash);

        let content_key = BlockHeaderContentKey { hash: block_hash };
        assert_eq!(content_key.hash.as_bytes(), raw_hash);
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
