use std::{
    fmt,
    net::IpAddr,
    str::{self, FromStr},
};

use rand::{distributions::Alphanumeric, prelude::Distribution, thread_rng};
use reqwest::{Client, Method, Url};
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
    #[error("Unknown tracker scheme '{0}'")]
    UnknownTrackerScheme(String),
}

pub struct Tracker {
    url: Url,
}

impl Tracker {
    pub fn new(url: Url) -> Tracker {
        Tracker { url }
    }

    pub async fn announce(&self, info: &Info) -> Result<TrackerResponse, TrackerError> {
        match self.url.scheme() {
            "http" | "https" => self.announce_http(info).await,
            "udp" => self.announce_udp(info).await,
            scheme => Err(TrackerError::UnknownTrackerScheme(scheme.to_string())),
        }
    }

    async fn announce_http(&self, info: &Info) -> Result<TrackerResponse, TrackerError> {
        let client = Client::new();

        let peer_id = Alphanumeric
            .sample_iter(&mut thread_rng())
            .take(20)
            .collect::<Vec<_>>();

        let peer_id =
            String::from_utf8(peer_id).expect("peer_id should always be valid alphanumeric ASCII");

        let info_hash = info.info_hash();

        let mut req = client
            .request(Method::GET, self.url.clone())
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

    async fn announce_udp(&self, _info: &Info) -> Result<TrackerResponse, TrackerError> {
        unimplemented!("implement UDP tracker")
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
    peers: Peers,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Peers {
    Dictionary(Vec<DictionaryPeer>),
    #[serde(deserialize_with = "deserialize_bytes_into_binary_peer_vec")]
    Binary(Vec<BinaryPeer>),
}

#[derive(Deserialize, Debug)]
pub struct DictionaryPeer {
    #[serde(rename = "peer id", with = "serde_bytes")]
    peer_id: Vec<u8>,
    #[serde(deserialize_with = "deserialize_bytes_to_peer_ip")]
    ip: PeerIp,
    port: u16,
}

#[derive(Debug)]
pub enum PeerIp {
    IpAddr(IpAddr),
    Dns(String),
}

#[derive(Debug)]
pub struct BinaryPeer {
    addr: IpAddr,
    port: u16,
}

fn deserialize_bytes_into_binary_peer_vec<'de, D>(
    deserializer: D,
) -> Result<Vec<BinaryPeer>, D::Error>
where
    D: Deserializer<'de>,
{
    struct BinaryPeersBytesVisitor;

    impl<'de> Visitor<'de> for BinaryPeersBytesVisitor {
        type Value = Vec<BinaryPeer>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("byte-string with a length multiple of 6")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.len() % 6 != 0 {
                return Err(E::custom("bytes not a multiple of 6"));
            }

            let chunks = v.chunks(6).map(|chunk| {
                let ip: [u8; 4] = chunk[0..4].try_into().unwrap();
                let port: [u8; 2] = chunk[5..6].try_into().unwrap();

                let addr = IpAddr::from(ip);
                let port = u16::from_be_bytes(port);

                BinaryPeer { addr, port }
            });

            Ok(chunks.collect())
        }
    }

    deserializer.deserialize_bytes(BinaryPeersBytesVisitor)
}

fn deserialize_bytes_to_peer_ip<'de, D>(deserializer: D) -> Result<PeerIp, D::Error>
where
    D: Deserializer<'de>,
{
    struct IpBytesVisitor;

    impl<'de> Visitor<'de> for IpBytesVisitor {
        type Value = PeerIp;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("exactly 4 or 16 bytes, or utf-8 representations of an IP address")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match str::from_utf8(v) {
                Ok(s) => match IpAddr::from_str(s) {
                    Ok(ip) => Ok(PeerIp::IpAddr(ip)),
                    Err(_) => Ok(PeerIp::Dns(s.to_string())),
                },
                Err(_) => Err(E::custom("invalid bytes in ip string representation")),
            }
        }
    }

    deserializer.deserialize_bytes(IpBytesVisitor)
}

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

#[cfg(test)]
mod tests {
    use crate::tracker::url_encode_bytes;

    #[test]
    fn test_byte_encode() {
        let bytes =
            b"\x12\x34\x56\x78\x9a\xbc\xde\xf1\x23\x45\x67\x89\xab\xcd\xef\x12\x34\x56\x78\x9a";
        let encoded = url_encode_bytes(bytes);

        assert_eq!(encoded, "%124Vx%9A%BC%DE%F1%23Eg%89%AB%CD%EF%124Vx%9A")
    }
}
