use std::{convert::TryFrom, io};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Error)]
pub enum PeerError {
    #[error("Invalid message type")]
    InvalidMessageId(u8),
    #[error("Incorrect message length for message type")]
    IncorrectMessageLen(u32),
    #[error("IO Error")]
    IoError(#[from] io::Error),
}

pub struct PeerCodec {
    am_choking: bool,
    am_interested: bool,
    peer_choking: bool,
    peer_interested: bool,
}

impl Default for PeerCodec {
    fn default() -> Self {
        Self {
            am_choking: true,
            am_interested: false,
            peer_choking: true,
            peer_interested: false,
        }
    }
}

#[repr(u8)]
enum PeerMessageKind {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

impl PeerMessageKind {
    pub fn msg_len(&self) -> Option<u32> {
        Some(match self {
            PeerMessageKind::Choke => 1,
            PeerMessageKind::Unchoke => 1,
            PeerMessageKind::Interested => 1,
            PeerMessageKind::NotInterested => 1,
            PeerMessageKind::Have => 5,
            PeerMessageKind::Request => 13,
            PeerMessageKind::Cancel => 13,
            PeerMessageKind::Bitfield | PeerMessageKind::Piece => return None,
        })
    }
}

impl TryFrom<u8> for PeerMessageKind {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => PeerMessageKind::Choke,
            1 => PeerMessageKind::Unchoke,
            2 => PeerMessageKind::Interested,
            3 => PeerMessageKind::NotInterested,
            4 => PeerMessageKind::Have,
            5 => PeerMessageKind::Bitfield,
            6 => PeerMessageKind::Request,
            7 => PeerMessageKind::Piece,
            8 => PeerMessageKind::Cancel,
            _ => return Err(value),
        })
    }
}

pub enum PeerMessage {
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        piece: Vec<u8>,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
    KeepAlive,
}

impl PeerMessage {
    fn kind(&self) -> Option<PeerMessageKind> {
        Some(match self {
            PeerMessage::Choke => PeerMessageKind::Choke,
            PeerMessage::Unchoke => PeerMessageKind::Unchoke,
            PeerMessage::Interested => PeerMessageKind::Interested,
            PeerMessage::NotInterested => PeerMessageKind::NotInterested,
            PeerMessage::Have(_) => PeerMessageKind::Have,
            PeerMessage::Bitfield(_) => PeerMessageKind::Bitfield,
            PeerMessage::Request { .. } => PeerMessageKind::Request,
            PeerMessage::Piece { .. } => PeerMessageKind::Piece,
            PeerMessage::Cancel { .. } => PeerMessageKind::Cancel,
            _ => return None,
        })
    }

    fn msg_len(&self) -> usize {
        match self {
            PeerMessage::KeepAlive => 0,
            PeerMessage::Choke => 1,
            PeerMessage::Unchoke => 1,
            PeerMessage::Interested => 1,
            PeerMessage::NotInterested => 1,
            PeerMessage::Have(_) => 5,
            PeerMessage::Request { .. } => 13,
            PeerMessage::Piece { piece, .. } => 9 + piece.len(),
            PeerMessage::Cancel { .. } => 13,
            PeerMessage::Bitfield(x) => 1 + x.len(),
        }
    }
}

impl Decoder for PeerCodec {
    type Item = PeerMessage;

    type Error = PeerError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // If no data is available wait for more
        if !src.has_remaining() {
            return Ok(None);
        }

        // Get the length prefix of message
        let len = src.get_u32();

        // Zero-length messages are keep-alive heartbeats
        if len == 0 {
            return Ok(Some(PeerMessage::KeepAlive));
        }

        // The message contains `len` bytes
        // Reserve them if they haven't arrived yet
        if src.remaining() < len as usize {
            src.reserve(len as usize);
            return Ok(None);
        }

        // Read the message ID
        let id = src.get_u8();

        // Validate message ID
        let message_kind = PeerMessageKind::try_from(id).map_err(PeerError::InvalidMessageId)?;

        // Find the static length of the message kind.
        // Bitfields and pieces don't have a fixed length, don't check those
        if let Some(message_len) = message_kind.msg_len() {
            if len != message_len {
                return Err(PeerError::IncorrectMessageLen(len));
            }
        }

        let message = match message_kind {
            PeerMessageKind::Choke => PeerMessage::Choke,
            PeerMessageKind::Unchoke => PeerMessage::Unchoke,
            PeerMessageKind::Interested => PeerMessage::Interested,
            PeerMessageKind::NotInterested => PeerMessage::NotInterested,
            PeerMessageKind::Have => {
                let piece = src.get_u32();
                PeerMessage::Have(piece)
            }
            PeerMessageKind::Bitfield => {
                let bits = src
                    .get(0..len as usize - 1)
                    .expect("Amount of remaining bytes has been checked")
                    .to_vec();

                PeerMessage::Bitfield(bits)
            }
            PeerMessageKind::Request => {
                let index = src.get_u32();
                let begin = src.get_u32();
                let length = src.get_u32();

                PeerMessage::Request {
                    index,
                    begin,
                    length,
                }
            }
            PeerMessageKind::Piece => {
                let index = src.get_u32();
                let begin = src.get_u32();

                let piece_size = len as usize - 9;

                let mut piece = Vec::with_capacity(piece_size);
                piece.put(src.take(piece_size));

                PeerMessage::Piece {
                    index,
                    begin,
                    piece,
                }
            }
            PeerMessageKind::Cancel => {
                let index = src.get_u32();
                let begin = src.get_u32();
                let length = src.get_u32();

                PeerMessage::Cancel {
                    index,
                    begin,
                    length,
                }
            }
        };

        Ok(Some(message))
    }
}

impl Encoder<PeerMessage> for PeerCodec {
    type Error = PeerError;

    fn encode(&mut self, item: PeerMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let len = item.msg_len();

        // Write length of peer message
        dst.put_u8(len as u8);

        let kind = match item.kind() {
            Some(kind) => kind,
            None => {
                // We're writing a keep-alive message with no more associated data
                return Ok(());
            }
        };

        // Kind is represented as the id byte
        let id = kind as u8;
        dst.put_u8(id);

        // Write the value-carrying variants' data
        match item {
            PeerMessage::Have(piece) => dst.put_u32(piece),
            PeerMessage::Bitfield(bits) => dst.extend_from_slice(&bits),
            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                dst.put_u32(index);
                dst.put_u32(begin);
                dst.put_u32(length);
            }
            PeerMessage::Piece {
                index,
                begin,
                piece,
            } => {
                dst.put_u32(index);
                dst.put_u32(begin);
                dst.put(Bytes::from(piece));
            }
            PeerMessage::Cancel {
                index,
                begin,
                length,
            } => {
                dst.put_u32(index);
                dst.put_u32(begin);
                dst.put_u32(length);
            }
            _ => {}
        }

        Ok(())
    }
}
