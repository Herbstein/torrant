use std::fmt;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};

#[derive(Debug, Deserialize)]
pub struct Torrent {
    announce: String,
    #[serde(rename = "announce-list")]
    announce_list: Vec<Vec<String>>,
    pub info: Info,
}

impl Torrent {
    pub fn announce(&self) -> &str {
        &self.announce
    }
}

#[derive(Deserialize, Serialize)]
pub struct Info {
    #[serde(flatten)]
    mode: FileMode,
    #[serde(rename = "piece length")]
    piece_length: usize,
    pieces: ByteBuf,
    #[serde(default)]
    private: bool,
}

impl fmt::Debug for Info {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Info")
            .field("mode", &self.mode)
            .field("piece_length", &self.piece_length)
            .field("pieces", &"<pieces>")
            .field("private", &self.private)
            .finish()
    }
}

impl Info {
    pub fn calculate_info_hash(&self) -> Result<[u8; 20]> {
        let bytes = bendy::serde::to_bytes(self)?;

        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        Ok(hasher.finalize().into())
    }

    pub fn length(&self) -> usize {
        match self.mode {
            FileMode::Single { length, .. } => length,
            FileMode::Multi { ref files, .. } => files.iter().map(|f| f.length).sum(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum FileMode {
    Single {
        name: String,
        length: usize,
        #[serde(skip)]
        md5sum: Option<String>,
    },
    Multi {
        name: String,
        files: Vec<File>,
        #[serde(skip)]
        md5sum: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct File {
    length: usize,
    path: Vec<String>,
}
