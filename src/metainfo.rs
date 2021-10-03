use std::fmt;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

#[derive(Debug, Deserialize, Serialize)]
pub struct Metainfo {
    announce: String,
    info: Info,
}

#[derive(Deserialize, Serialize)]
pub struct Info {
    name: String,
    #[serde(rename = "piece length")]
    piece_length: usize,
    #[serde(with = "serde_bytes")]
    pieces: Vec<u8>,
    #[serde(flatten)]
    key: Key,
}

impl Info {
    /// Calculate SHA-1 info hash
    pub fn info_hash(&self) -> Result<Vec<u8>> {
        let data = serde_bencode::to_bytes(self).context("failed to serialize info struct")?;

        let mut hasher = Sha1::new();
        hasher.update(&data);

        let res = hasher.finalize();

        Ok(res.as_slice().to_vec())
    }
}

impl fmt::Debug for Info {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Info")
            .field("name", &self.name)
            .field("piece_length", &self.piece_length)
            .field("key", &self.key)
            .finish()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Key {
    KeyLength { length: usize },
    KeyFiles { files: Vec<File> },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct File {
    length: usize,
    path: Vec<String>,
}
