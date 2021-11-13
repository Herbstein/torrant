use std::{error, fmt::Write, io::Read};

use bytes::Buf;
use rand::{distributions::Alphanumeric, prelude::Distribution, thread_rng};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
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
            .take(10)
            .collect::<Vec<_>>();

        let peer_id =
            String::from_utf8(peer_id).expect("peer_id should always be valid alphanumeric ASCII");

        let info_hash = info.info_hash();
        let info_hash_encoded = urlencoding::encode_binary(&info_hash).into_owned();

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

        // `.query()` on the RequestBuilder double-encodes the info_hash encoding
        // Manually append to avoid
        req.url_mut()
            .query_pairs_mut()
            .append_pair("info_hash", &url_encode_bytes(&info_hash).expect("err2"));

        let resp = client
            .execute(req)
            .await
            .map_err(|_| TrackerError::AnnounceRequestFailed(self.url.to_string()))?;

        let body = resp
            .bytes()
            .await
            .map_err(|_| TrackerError::NoBodyInTrackerResponse)?;

        let mut vec_body = Vec::new();
        body.reader().read_to_end(&mut vec_body).expect("err");

        let body = vec_body.as_slice();

        let body_str = std::str::from_utf8(body);

        let tracker_result = serde_bencode::from_bytes(body)
            .map_err(|_| TrackerError::InvalidBodyInTrackerResponse)?;

        let tracker_response = match tracker_result {
            TrackerResult::Failure { failure_reason } => {
                return Err(TrackerError::TrackerReturnedError(failure_reason))
            }
            TrackerResult::Success { interval, peers } => TrackerResponse { interval, peers },
        };

        Ok(tracker_response)
    }
}

pub fn url_encode_bytes(content: &[u8]) -> Result<String, Box<dyn error::Error>> {
    let mut out = String::new();

    for byte in content.iter() {
        match *byte as char {
            c @ ('0'..='9' | 'a'..='z' | 'A'..='Z' | '.' | '-' | '_' | '~') => out.push(c),
            _ => write!(&mut out, "%{:02X}", byte)?,
        }
    }

    Ok(out)
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum TrackerResult {
    Failure {
        #[serde(rename = "failure reason")]
        failure_reason: String,
    },
    Success {
        interval: usize,
        peers: Vec<Peer>,
    },
}

#[derive(Debug)]
pub struct TrackerResponse {
    interval: usize,
    peers: Vec<Peer>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Peer {
    peer_id: String,
    ip: String,
    port: u16,
}
