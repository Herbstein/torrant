use std::io;

use bytes::{Buf, BytesMut};
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid message type")]
    InvalidMessageId(u8),
    #[error("Incorrect message length for message type")]
    IncorrectMessageLen(u8),
    #[error("IO Error")]
    Other(#[from] io::Error),
}

pub struct PeerProtocol;

pub enum PeerMessage {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request { index: u32, begin: u32, length: u32 },
    Piece { index: u32, begin: u32, piece: u32 },
    Cancel { index: u32, begin: u32, length: u32 },
    KeepAlive,
}

impl Decoder for PeerProtocol {
    type Item = PeerMessage;

    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // If no data is available wait for more
        if !src.has_remaining() {
            return Ok(None);
        }

        // Get the length prefix of message
        let len = src.get_u8();

        // Zero-length messages are keep-alive heartbeats
        if len == 0 {
            return Ok(Some(PeerMessage::KeepAlive));
        }

        // The next message contains `len` bytes
        // Reserve them if they haven't arrived yet
        if src.remaining() < len as usize {
            src.reserve(len as usize);
            return Ok(None);
        }

        // Message ID
        let id = src.get_u8();

        let message = match id {
            0 => {
                if len != 1 {
                    return Err(Error::IncorrectMessageLen(len));
                }
                PeerMessage::Choke
            }
            1 => {
                if len != 1 {
                    return Err(Error::IncorrectMessageLen(len));
                }
                PeerMessage::Unchoke
            }
            2 => {
                if len != 1 {
                    return Err(Error::IncorrectMessageLen(len));
                }
                PeerMessage::Interested
            }
            3 => {
                if len != 1 {
                    return Err(Error::IncorrectMessageLen(len));
                }
                PeerMessage::NotInterested
            }
            4 => {
                if len != 5 {
                    return Err(Error::IncorrectMessageLen(len));
                }

                let piece = src.get_u32();

                PeerMessage::Have(piece)
            }
            5 => {
                let bits = src
                    .get(0..len as usize - 1)
                    .expect("Amount of remaining bytes has been checked")
                    .to_vec();

                PeerMessage::Bitfield(bits)
            }
            6 => {
                if len != 13 {
                    return Err(Error::IncorrectMessageLen(len));
                }

                let index = src.get_u32();
                let begin = src.get_u32();
                let length = src.get_u32();

                PeerMessage::Request {
                    index,
                    begin,
                    length,
                }
            }
            7 => {
                if len != 13 {
                    return Err(Error::IncorrectMessageLen(len));
                }

                let index = src.get_u32();
                let begin = src.get_u32();
                let piece = src.get_u32();

                PeerMessage::Piece {
                    index,
                    begin,
                    piece,
                }
            }
            8 => {
                if len != 13 {
                    return Err(Error::IncorrectMessageLen(len));
                }

                let index = src.get_u32();
                let begin = src.get_u32();
                let length = src.get_u32();

                PeerMessage::Cancel {
                    index,
                    begin,
                    length,
                }
            }
            _ => return Err(Error::InvalidMessageId(id)),
        };

        Ok(Some(message))
    }
}

impl Encoder<PeerMessage> for PeerProtocol {
    type Error = Error;

    fn encode(&mut self, item: PeerMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        todo!()
    }
}
