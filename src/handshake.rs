use std::io;

use bytes::BytesMut;
use thiserror::Error;
use tokio_util::codec::Decoder;

#[derive(Debug, Error)]
enum Error {
    #[error("Some IO error")]
    IoError(#[from] io::Error),
}

pub struct HandshakeProtocol;

struct Handshake {
    text: [u8; 19],
    reserved: [u8; 8],
    hash: [u8; 20],
    peer_id: [u8; 20],
}

impl Decoder for Handshake {
    type Item = Handshake;

    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        todo!()
    }
}
