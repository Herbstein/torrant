pub mod handshake;
pub mod metainfo;
pub mod peer;
pub mod tracker;
pub mod util;

pub type InfoHash = [u8; 20];

pub use reqwest::Url;

pub const VERSION_NUMBER: [u8; 5] = *b"00000";
