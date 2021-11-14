use std::{
    fmt,
    net::IpAddr,
    str::{self, FromStr},
};

use rand::{distributions::Alphanumeric, prelude::Distribution, thread_rng};
use reqwest::{Client, Method};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};
use thiserror::Error;

use crate::metainfo::Info;

#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("Tracker HTTP request to '{0}' failed")]
    AnnounceRequestFailed(String, reqwest::Error),
    #[error("Error when building announce request")]
    BuildingRequestFailed,
    #[error("Tracker didn't return a request")]
    NoBodyInTrackerResponse(reqwest::Error),
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

        // `.query()` on the RequestBuilder re-encodes the already encoded info_hash
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
            .map_err(|err| TrackerError::AnnounceRequestFailed(self.url.to_string(), err))?;

        let body = resp
            .bytes()
            .await
            .map_err(TrackerError::NoBodyInTrackerResponse)?;

        let tracker_result = serde_bencode::from_bytes(&body).unwrap();

        let tracker_response = match tracker_result {
            TrackerResult::Failure { failure_reason } => {
                return Err(TrackerError::TrackerReturnedError(
                    String::from_utf8(failure_reason).unwrap(),
                ))
            }
            TrackerResult::Success(response) => response,
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
    Success(TrackerResponse),
}

#[derive(Deserialize, Debug)]
pub struct TrackerResponse {
    complete: usize,
    incomplete: usize,
    interval: usize,
    peers: Vec<Peer>,
}

// TODO: Custom from-bytes... conversion into Ipv4Addr
#[derive(Deserialize, Debug)]
pub struct Peer {
    #[serde(rename = "peer id", with = "serde_bytes")]
    peer_id: Vec<u8>,
    #[serde(deserialize_with = "deserialize_bytes_to_ipv4_addr")]
    ip: IpAddr,
    port: u16,
}

fn deserialize_bytes_to_ipv4_addr<'de, D>(deserializer: D) -> Result<IpAddr, D::Error>
where
    D: Deserializer<'de>,
{
    struct IpBytesVisitor;

    impl<'de> Visitor<'de> for IpBytesVisitor {
        type Value = IpAddr;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("exactly 4 or 16 bytes, or utf-8 representations of an IP address")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match v.len() {
                4 => {
                    // Raw IPv4 bytes
                    let bytes: [u8; 4] = v.try_into().unwrap();
                    Ok(IpAddr::from(bytes))
                }
                16 => {
                    // Raw IPv6 bytes
                    let bytes: [u8; 16] = v.try_into().unwrap();
                    Ok(IpAddr::from(bytes))
                }
                _ => {
                    // assume string representation, error otherwise
                    match str::from_utf8(v) {
                        Ok(s) => IpAddr::from_str(s).map_err(|e| E::custom(e.to_string())),
                        Err(_) => Err(E::custom("invalid bytes in ip string representation")),
                    }
                }
            }
        }
    }

    deserializer.deserialize_byte_buf(IpBytesVisitor)
}
