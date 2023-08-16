use std::io::{self, Cursor};

use anyhow::Result;
use byteorder::{BigEndian, ReadBytesExt};
use bytes::BytesMut;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, ToSocketAddrs},
};
use tokio_util::codec::Decoder;

pub enum PeerMessage {
    KeepAlive,
}

pub struct PeerCodec {
    am_choking: bool,
    peer_choking: bool,
    am_interested: bool,
    peer_interested: bool,
}

impl Decoder for PeerCodec {
    type Item = PeerMessage;

    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            src.reserve(4 - src.len());
            return Ok(None);
        }

        let mut rdr = Cursor::new(src);

        let len = ReadBytesExt::read_u32::<BigEndian>(&mut rdr)?;
        match len {
            0 => Ok(Some(PeerMessage::KeepAlive)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "only peer message 'keep-alive' recognized",
            )),
        }
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

    Ok(())
}
