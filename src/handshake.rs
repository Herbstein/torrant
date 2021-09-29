use std::io;

use bytes::{Buf, BufMut, BytesMut};
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder};

use crate::util::ReadExactExt;

const PROTOCOL_NAME: &[u8] = b"BitTorrentprotocol";

#[derive(Debug, Error)]
pub enum Error {
    #[error("Wrong prefix ({0}) supplied")]
    WrongPrefix(u8),
    #[error("The string 'BitTorrent protocol' not encountered in handshake")]
    NoProtocolText,
    #[error("Some IO error")]
    IoError(#[from] io::Error),
}

pub struct HandshakeProtocol;

pub struct Handshake {
    text: [u8; 19],
    reserved: [u8; 8],
    hash: [u8; 20],
    peer_id: [u8; 20],
}

impl Decoder for HandshakeProtocol {
    type Item = Handshake;

    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Header has static size. Don't start parsing until a headers worth of bytes have been received
        if src.remaining() < 68 {
            src.reserve(68);
            return Ok(None);
        }

        let prefix = src.get_u8();
        if prefix != 19 {
            return Err(Error::WrongPrefix(prefix));
        }

        let text = match src.read_exact() {
            Some(text) => text,
            None => return Ok(None),
        };
        if text != PROTOCOL_NAME {
            return Err(Error::NoProtocolText);
        }

        let reserved = match src.read_exact() {
            Some(reserved) => reserved,
            None => return Ok(None),
        };

        let hash = match src.read_exact() {
            Some(hash) => hash,
            None => return Ok(None),
        };

        let peer_id = match src.read_exact() {
            Some(peer_id) => peer_id,
            None => return Ok(None),
        };

        Ok(Some(Handshake {
            text,
            reserved,
            hash,
            peer_id,
        }))
    }
}

impl Encoder<Handshake> for HandshakeProtocol {
    type Error = Error;

    fn encode(&mut self, item: Handshake, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // Header has static size
        dst.reserve(67);

        // Protocol name length
        dst.put_u8(19);

        // Protocol name
        dst.put(PROTOCOL_NAME);

        // Reserved bytes
        dst.put_bytes(0, 8);

        // Info hash
        dst.put(item.hash.as_ref());

        // Peer ID
        dst.put(item.peer_id.as_ref());

        Ok(())
    }
}
