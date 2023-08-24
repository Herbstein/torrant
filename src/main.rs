use std::{io::Read, net::Ipv4Addr};

use anyhow::Result;
use rand::{thread_rng, Rng, RngCore};
use reqwest::Client;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use tokio::{fs::OpenOptions, io::AsyncReadExt};

use crate::info::Torrent;

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
        .open("data/test1.torrent")
        .await
        .unwrap();

    let mut data = Vec::new();
    file.read_to_end(&mut data).await?;

    let torrent = bendy::serde::from_bytes::<Torrent>(&data).unwrap();

    let info_hash = torrent.info.calculate_info_hash()?;
    let info_hash_formencoded = form_encode(&info_hash);

    let peer_id = generate_peer_id();
    let peer_id_formencoded = form_encode(&peer_id);

    println!("{:x?}", peer_id);
    println!("{:?}", peer_id_formencoded);

    let left = torrent.info.length();

    let client = Client::new();

    let mut req = client.get(torrent.announce()).build()?;
    req.url_mut().set_query(Some(&format!(
       "info_hash={info_hash_formencoded}&peer_id={peer_id_formencoded}&port=6881&uploaded=0&downloaded=0&left={left}&event=started&compact=1"
    )));

    println!("{:?}", req.url());

    let resp = client.execute(req).await?;
    let body = resp.bytes().await?;

    let tracker_response = bendy::serde::from_bytes::<CompactTrackerResponse>(&body)?;
    assert!(tracker_response.peers.len() % 6 == 0);

    let peers = tracker_response
        .peers
        .chunks_exact(6)
        .map(|x| {
            let mut ip = [0; 4];
            ip.copy_from_slice(&x[..4]);
            let ip = Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]);

            let mut port = [0; 2];
            port.copy_from_slice(&x[4..]);
            let port = u16::from_be_bytes(port);

            (ip, port)
        })
        .collect::<Vec<_>>();
    println!("{peers:?}");

    let connect_futures = peers
        .iter()
        .map(|(ip, port)| peer::connect(info_hash, peer_id, (*ip, *port)));

    futures::future::join_all(connect_futures)
        .await
        .into_iter()
        .for_each(|r| println!("{r:?}"));

    Ok(())
}
