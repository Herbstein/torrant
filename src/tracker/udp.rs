use std::net::{IpAddr, Ipv4Addr};

use binread::BinRead;
use binwrite::BinWrite;

use crate::tracker::{BinaryPeer, Peers, TrackerResponse};

#[derive(BinWrite)]
#[binwrite(big)]
pub struct TrackerHandshakeRequest {
    connection_id: i64,
    action: i32,
    transaction_id: i32,
}

impl TrackerHandshakeRequest {
    pub fn new(transaction_id: i32) -> Self {
        Self {
            connection_id: 0x41727101980,
            action: 0,
            transaction_id,
        }
    }
}

#[derive(BinRead)]
#[br(big)]
pub struct TrackerHandshakeResponse {
    pub action: i32,
    pub transaction_id: i32,
    pub connection_id: i64,
}

#[derive(BinWrite)]
pub struct TrackerAnnounceRequest {
    connection_id: i64,
    action: i32,
    transaction_id: i32,
    info_hash: [u8; 20],
    peer_id: [u8; 20],
    downloaded: i64,
    left: i64,
    uploaded: i64,
    event: i32,
    ip: u32,
    key: u32,
    num_want: i32,
    port: u16,
    extensions: u16,
}

impl TrackerAnnounceRequest {
    pub fn new(
        connection_id: i64,
        transaction_id: i32,
        info_hash: [u8; 20],
        peer_id: [u8; 20],
        downloaded: i64,
        left: i64,
        uploaded: i64,
        event: Event,
        ip: Option<u32>,
        key: u32,
        port: u16,
    ) -> Self {
        Self {
            connection_id,
            action: 1,
            transaction_id,
            info_hash,
            peer_id,
            downloaded,
            left,
            uploaded,
            event: event as i32,
            ip: ip.unwrap_or_default(),
            key,
            num_want: -1,
            port,
            extensions: 0,
        }
    }
}

#[derive(BinRead)]
#[br(assert(action == 1))]
pub struct TrackerAnnounceResponse {
    action: i32,
    pub transaction_id: i32,
    interval: i32,
    leechers: i32,
    seeders: i32,
    #[br(count = leechers+seeders)]
    peers: Vec<Peer>,
}

impl From<TrackerAnnounceResponse> for TrackerResponse {
    fn from(resp: TrackerAnnounceResponse) -> Self {
        Self {
            complete: 0,
            incomplete: resp.seeders as usize,
            interval: resp.interval as usize,
            peers: Peers::Binary(
                resp.peers
                    .into_iter()
                    .map(|p| BinaryPeer {
                        addr: IpAddr::V4(Ipv4Addr::from(p.ip as u32)),
                        port: p.port,
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(BinRead)]
pub struct Peer {
    ip: i32,
    port: u16,
}

#[repr(i32)]
pub enum Event {
    None = 0,
    Completed = 1,
    Started = 2,
    Stopped = 3,
}
