use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::InfoHash;

#[derive(Debug, Deserialize, Serialize)]
pub struct Metainfo {
    announce: String,
    info: Info,
}

impl Metainfo {
    pub fn info(&self) -> &Info {
        &self.info
    }

    pub fn announce_url(&self) -> &str {
        &self.announce
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Info {
    name: String,
    #[serde(rename = "piece length")]
    piece_length: usize,
    #[serde(with = "pieces_serde")]
    pieces: Pieces,
    #[serde(flatten)]
    key: Key,
}

impl Info {
    /// Calculate SHA-1 info hash
    pub fn info_hash(&self) -> Vec<u8> {
        // `expect`ing here is fine because the serializer is infallible and no floating point numbers are used in the protocol
        let data = serde_bencode::to_bytes(self).expect("piece serialization is broken");

        let mut hasher = Sha1::new();
        hasher.update(&data);

        let info_hash = hasher.finalize();

        info_hash.to_vec()
    }

    pub fn total_bytes(&self) -> usize {
        self.piece_length * self.pieces.count()
    }
}

#[derive(Debug)]
pub struct Pieces(Vec<Piece>);

impl Pieces {
    pub fn count(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug)]
pub struct Piece(InfoHash);

mod pieces_serde {
    use std::{convert::TryInto, fmt};

    use serde::{
        de::{self, Error, Visitor},
        Deserializer, Serializer,
    };

    use crate::metainfo::{Piece, Pieces};

    pub fn deserialize<'de, D>(d: D) -> Result<Pieces, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ByteVisitor;

        impl<'de> Visitor<'de> for ByteVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a byte sequence")
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(v)
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(v.to_vec())
            }
        }

        let bytes = d.deserialize_byte_buf(ByteVisitor)?;

        if bytes.len() % 20 != 0 {
            return Err(Error::custom("Expected factor 20-sized buffer"));
        }

        let pieces = bytes
            .chunks(20)
            .map(|x| Piece(x.try_into().unwrap()))
            .collect();

        Ok(Pieces(pieces))
    }

    pub fn serialize<S>(v: &Pieces, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut out = Vec::new();
        for p in &v.0 {
            out.extend_from_slice(&p.0);
        }
        s.serialize_bytes(&out)
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
