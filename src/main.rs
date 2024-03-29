use std::{net::Ipv4Addr, time::Duration};

use anyhow::Result;
use futures::{SinkExt, StreamExt};
use rand::{thread_rng, RngCore};
use reqwest::Client;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use tokio::{fs::OpenOptions, io::AsyncReadExt};

use crate::{info::Torrent, peer::PeerMessage};

mod info;
mod peer;
mod tracker;

fn form_encode(b: &[u8]) -> String {
    url::form_urlencoded::byte_serialize(b)
        .map(|x| if x == "+" { "%20" } else { x })
        .collect()
}

fn generate_peer_id() -> [u8; 20] {
    let mut rng = thread_rng();

    let client = b"TOR0001-";
    let mut peer = [0; 12];
    rng.fill_bytes(&mut peer);

    let mut out = [0; 20];
    out[..8].copy_from_slice(client);
    out[8..].copy_from_slice(&peer);

    out
}

#[derive(Debug, Deserialize)]
struct CompactTrackerResponse {
    interval: usize,
    peers: ByteBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .open("data/test3.torrent")
        .await
        .unwrap();

    let mut data = Vec::new();
    file.read_to_end(&mut data).await?;

    let torrent = bendy::serde::from_bytes::<Torrent>(&data).unwrap();

    let info_hash = torrent.info.calculate_info_hash()?;
    let info_hash_formencoded = form_encode(&info_hash);

    let peer_id = generate_peer_id();
    let peer_id_formencoded = form_encode(&peer_id);

    let left = torrent.info.length();

    let client = Client::new();

    // let mut req = client.get(torrent.announce()).build()?;
    // req.url_mut().set_query(Some(&format!(
    //    "info_hash={info_hash_formencoded}&peer_id={peer_id_formencoded}&port=6881&uploaded=0&downloaded=0&left={left}&event=started&compact=1"
    // )));

    // let resp = client.execute(req).await?;
    // let body = resp.bytes().await?;

    // let tracker_response = bendy::serde::from_bytes::<CompactTrackerResponse>(&body)?;
    // assert!(tracker_response.peers.len() % 6 == 0);

    // let peers = tracker_response
    //     .peers
    //     .chunks_exact(6)
    //     .map(|x| {
    //         let mut ip = [0; 4];
    //         ip.copy_from_slice(&x[..4]);
    //         let ip = Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]);

    //         let mut port = [0; 2];
    //         port.copy_from_slice(&x[4..]);
    //         let port = u16::from_be_bytes(port);

    //         (ip, port)
    //     })
    //     .collect::<Vec<_>>();
    // println!("{:?}", peers);

    let mut buffer = vec![0; torrent.info.length()];

    let framed = peer::connect(info_hash, peer_id, ("localhost", 16355)).await?;
    let (mut writer, mut reader) = framed.split();

    writer.send(PeerMessage::Interested).await?;
    // writer.send(PeerMessage::Unchoke).await?;

    let mut current_piece = 0;

    while let Some(Ok(data)) = reader.next().await {
        // println!("{data:x?}");

        let total_full_pieces = torrent.info.length() / torrent.info.piece_length();

        match data {
            PeerMessage::Unchoke => {
                writer
                    .send(PeerMessage::Request(
                        current_piece,
                        0,
                        torrent.info.piece_length() as u32,
                    ))
                    .await?
            }
            PeerMessage::Piece(piece_index, block_index, block_data) => {
                println!("Received {} bytes in block", block_data.len());

                let start_idx =
                    piece_index as usize * torrent.info.piece_length() + block_index as usize;
                buffer.splice(start_idx..start_idx + block_data.len(), block_data);

                writer.send(PeerMessage::Have(current_piece)).await?;

                current_piece += 1;

                if current_piece < total_full_pieces as u32 {
                    writer
                        .send(PeerMessage::Request(
                            current_piece,
                            0,
                            torrent.info.piece_length() as u32,
                        ))
                        .await?;
                } else if current_piece == total_full_pieces as u32 {
                    writer
                        .send(PeerMessage::Request(
                            current_piece,
                            0,
                            (torrent.info.length() % torrent.info.piece_length()) as u32,
                        ))
                        .await?;
                } else {
                    println!("Received all bytes!");
                }
            }
            _ => {}
        }
    }

    // let connect_futures = peers
    //     .iter()
    //     .map(|(ip, port)| peer::connect(info_hash, peer_id, (*ip, *port)));
    //
    // futures::future::join_all(connect_futures)
    //     .await
    //     .into_iter()
    //     .for_each(|r| println!("{r:?}"));

    Ok(())
}
