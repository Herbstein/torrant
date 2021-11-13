use std::{error, fmt::Write, net::Ipv4Addr};

use rand::{distributions::Alphanumeric, prelude::Distribution, thread_rng};
use reqwest::{Client, Method};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::metainfo::Info;

#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("Tracker HTTP request to '{0}' failed")]
    AnnounceRequestFailed(String),
    #[error("Error when building announce request")]
    BuildingRequestFailed,
    #[error("Tracker didn't return a request")]
    NoBodyInTrackerResponse,
    #[error("Tracker response wasn't valid")]
    InvalidBodyInTrackerResponse,
    #[error("Announce failed with reason '{0}'")]
    TrackerReturnedError(String),
}

pub struct Tracker {
    url: String,
}

impl Tracker {
    pub fn new(url: String) -> Tracker {
        Tracker { url }
    }

    pub async fn announce(&self, info: &Info) -> Result<TrackerResponse, TrackerError> {
        let client = Client::new();

        let peer_id = Alphanumeric
            .sample_iter(&mut thread_rng())
            .take(20)
            .collect::<Vec<_>>();

        let peer_id =
            String::from_utf8(peer_id).expect("peer_id should always be valid alphanumeric ASCII");

        let info_hash = info.info_hash();

        let mut req = client
            .request(Method::GET, &self.url)
            .query(&[
                ("peer_id", peer_id.as_str()),
                // ("ip", ""), <-- optional. wanna use anyway?
                ("port", "6881"),
                ("uploaded", "0"),
                ("downloaded", "0"),
                ("left", &info.total_bytes().to_string()),
            ])
            .build()
            .map_err(|_| TrackerError::BuildingRequestFailed)?;

        fn url_encode_bytes(content: &[u8]) -> String {
            let mut out = String::new();

            for byte in content.iter() {
                match *byte as char {
                    c @ ('0'..='9' | 'a'..='z' | 'A'..='Z' | '.' | '-' | '_' | '~') => out.push(c),
                    ' ' => out.push('+'),
                    _ => out.push_str(&format!("%{:02X}", byte)),
                }
            }

            out
        }

        // `.query()` on the RequestBuilder double-encodes the info_hash encoding
        // Manually append to avoid
        let query_url = req.url().query().expect("query URL definitely set");
        let query_url = format!(
            "{}&{}={}",
            query_url,
            "info_hash",
            url_encode_bytes(&info_hash)
        );
        req.url_mut().set_query(Some(&query_url));

        let resp = client
            .execute(req)
            .await
            .map_err(|_| TrackerError::AnnounceRequestFailed(self.url.to_string()))?;

        let body = resp
            .bytes()
            .await
            .map_err(|_| TrackerError::NoBodyInTrackerResponse)?;

        let tracker_result = serde_bencode::from_bytes(&body).unwrap();

        let tracker_response = match tracker_result {
            TrackerResult::Failure { failure_reason } => {
                return Err(TrackerError::TrackerReturnedError(
                    String::from_utf8(failure_reason).unwrap(),
                ))
            }
            TrackerResult::Success {
                interval, peers, ..
            } => TrackerResponse { interval, peers },
        };

        Ok(tracker_response)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TrackerResult {
    Failure {
        #[serde(rename = "failure reason")]
        failure_reason: Vec<u8>,
    },
    Success {
        complete: usize,
        incomplete: usize,
        interval: usize,
        peers: Vec<Peer>,
    },
}

#[derive(Debug)]
pub struct TrackerResponse {
    interval: usize,
    peers: Vec<Peer>,
}

// TODO: Custom byte-to... conversion for IP
#[derive(Deserialize, Debug)]
pub struct Peer {
    #[serde(rename = "peer id", with = "serde_bytes")]
    peer_id: Vec<u8>,
    #[serde(with = "serde_bytes")]
    ip: Vec<u8>,
    port: u16,
}
