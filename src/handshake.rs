use std::{convert::TryInto, io};

use bytes::{Buf, BytesMut};
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder};

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
        if src.remaining() < 67 {
            src.reserve(67);
            return Ok(None);
        }

        let prefix = src.get_u8();
        if prefix != 19 {
            return Err(Error::WrongPrefix(prefix));
        }

        let text = src
            .get(0..19)
            .expect("Infallible because `remaining` has already been checked")
            .try_into()
            .expect("The exact right amount of bytes were read");
        if &text != b"BitTorrent protocol" {
            return Err(Error::NoProtocolText);
        }

        let reserved = src
            .get(..8)
            .expect("Infallible because `remaining` has already been checked")
            .try_into()
            .expect("The exact right amount of bytes were read");

        let hash = src
            .get(..20)
            .expect("Infallible because `remaining` has already been checked")
            .try_into()
            .expect("The exact right amount of bytes were read");

        let peer_id = src
            .get(..20)
            .expect("Infallible because `remaining` has already been checked")
            .try_into()
            .expect("The exact right amount of bytes were read");

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
        todo!()
    }
}
