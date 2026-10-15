//! Domain-separated SHA-256 for statechain commitments.

use sha2::{Digest, Sha256};

pub type StateHash = [u8; 32];

pub fn tagged_hash(tag: &str, parts: &[&[u8]]) -> StateHash {
    let mut hasher = Sha256::new();
    hasher.update(tag.as_bytes());
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

pub fn hash_pair(tag: &str, left: &StateHash, right: &StateHash) -> StateHash {
    tagged_hash(tag, &[left, right])
}
