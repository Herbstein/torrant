use std::io;

use bytes::BytesMut;
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO Error")]
    Other(#[from] io::Error),
}

pub struct PeerProtocol;

pub enum PeerMessage {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have,
    Bitfield,
    Request,
    Piece,
    Cancel,
}

impl Decoder for PeerProtocol {
    type Item = PeerMessage;

    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        todo!()
    }
}

impl Encoder<PeerMessage> for PeerProtocol {
    type Error = Error;

    fn encode(&mut self, item: PeerMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        todo!()
    }
}
