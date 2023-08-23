use std::io;

use anyhow::{Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use futures::{SinkExt, StreamExt};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, ToSocketAddrs},
};
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite};

#[derive(Debug)]
pub enum PeerMessage {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request(u32, u32, u32),
    Piece(u32, u32, Vec<u8>),
    Cancel(u32, u32, u32),
}

pub struct PeerCodec {
    am_choking: bool,
    peer_choking: bool,
    am_interested: bool,
    peer_interested: bool,
}

impl PeerCodec {
    pub fn new() -> Self {
        Self {
            am_choking: true,
            am_interested: false,
            peer_choking: true,
            peer_interested: false,
        }
    }
}

macro_rules! read_const_bytes {
    ($src:expr,  $start:expr, $len:expr) => {{
        let mut data = [0; $len];
        data.copy_from_slice(&$src[$start..$start + $len]);
        data
    }};
    ($src:expr, $start:expr, $len:expr, $converter:expr) => {{
        let data = read_const_bytes!($src, $start, $len);
        $converter(data)
    }};
}

macro_rules! read_u32 {
    ($src:expr, $start:expr) => {
        read_const_bytes!($src, $start, 4, u32::from_be_bytes)
    };
}

impl Encoder<PeerMessage> for PeerCodec {
    type Error = io::Error;

    fn encode(&mut self, item: PeerMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let len = match item {
            PeerMessage::KeepAlive => 0,
            PeerMessage::Choke => 1,
            PeerMessage::Unchoke => 1,
            PeerMessage::Interested => 1,
            PeerMessage::NotInterested => 1,
            PeerMessage::Have(_) => 1 + 4,
            PeerMessage::Bitfield(ref bitfield) => 1 + bitfield.len() as u32,
            PeerMessage::Request(_, _, _) => 13,
            PeerMessage::Piece(_, _, ref block) => 9 + block.len() as u32,
            PeerMessage::Cancel(_, _, _) => 13,
        };

        dst.put_u32(len);

        let id = match item {
            PeerMessage::KeepAlive => None,
            PeerMessage::Choke => Some(0),
            PeerMessage::Unchoke => Some(1),
            PeerMessage::Interested => Some(2),
            PeerMessage::NotInterested => Some(3),
            PeerMessage::Have(_) => Some(4),
            PeerMessage::Bitfield(_) => Some(5),
            PeerMessage::Request(_, _, _) => Some(6),
            PeerMessage::Piece(_, _, _) => Some(7),
            PeerMessage::Cancel(_, _, _) => Some(8),
        };

        if let Some(id) = id {
            dst.put_u8(id);
        }

        match item {
            PeerMessage::Have(piece_index) => dst.put_u32(piece_index),
            PeerMessage::Bitfield(bitfield) => dst.put_slice(&bitfield),
            PeerMessage::Request(piece_index, block_index, block_length) => {
                dst.put_u32(piece_index);
                dst.put_u32(block_index);
                dst.put_u32(block_length);
            }
            PeerMessage::Piece(piece_index, block_index, ref block) => {
                dst.put_u32(piece_index);
                dst.put_u32(block_index);
                dst.put_slice(block);
            }
            PeerMessage::Cancel(piece_index, block_index, block_length) => {
                dst.put_u32(piece_index);
                dst.put_u32(block_index);
                dst.put_u32(block_length);
            }
            _ => {}
        }

        Ok(())
    }
}

impl Decoder for PeerCodec {
    type Item = PeerMessage;

    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            src.reserve(4 - src.len());
            return Ok(None);
        }

        let mut len = [0; 4];
        len.copy_from_slice(&src[0..4]);
        let len = u32::from_be_bytes(len);

        if len == 0 {
            src.advance(4);
            return Ok(Some(PeerMessage::KeepAlive));
        }

        if src.len() < len as usize {
            src.reserve(len as usize - src.len());
            return Ok(None);
        }

        let mut id = [0; 1];
        id.copy_from_slice(&src[4..5]);
        let id = u8::from_be_bytes(id);

        let x = match id {
            0 => PeerMessage::Choke,
            1 => PeerMessage::Unchoke,
            2 => PeerMessage::Interested,
            3 => PeerMessage::NotInterested,
            4 => PeerMessage::Have(read_u32!(src, 5)),
            5 => {
                let bitfield_length = len - 1;
                let bitfield = src[5..bitfield_length as usize].to_vec();
                PeerMessage::Bitfield(bitfield)
            }
            6 => PeerMessage::Request(read_u32!(src, 5), read_u32!(src, 9), read_u32!(src, 13)),
            7 => {
                let block_length = len - 9;
                PeerMessage::Piece(
                    read_u32!(src, 5),
                    read_u32!(src, 9),
                    src[13..block_length as usize].to_vec(),
                )
            }
            8 => PeerMessage::Cancel(read_u32!(src, 5), read_u32!(src, 9), read_u32!(src, 13)),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("only peer message 'keep-alive' recognized. got id of {id}"),
                ))
            }
        };

        src.advance(4 + len as usize);

        Ok(Some(x))
    }
}

pub async fn connect(
    info_hash: [u8; 20],
    peer_id: [u8; 20],
    addr: impl ToSocketAddrs,
) -> Result<()> {
    let mut stream = TcpStream::connect(addr).await?;
    stream.write_u8(19).await?;
    stream.write_all(b"BitTorrent protocol").await?;
    stream.write_u64(0).await?;
    stream.write_all(&info_hash).await?;
    stream.write_all(&peer_id).await?;

    let mut handshake_recv = [0; 68];
    stream.read_exact(&mut handshake_recv).await?;

    assert_eq!(handshake_recv[0], 19);
    assert_eq!(&handshake_recv[1..20], b"BitTorrent protocol");
    // Reserved bits
    // assert_eq!(&handshake_recv[20..28], &[0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&handshake_recv[28..48], &info_hash);

    let (reader, writer) = stream.into_split();

    let mut frames = FramedRead::new(reader, PeerCodec::new());

    while let Some(Ok(data)) = frames.next().await {
        println!("{data:x?}");
    }

    Ok(())
}
