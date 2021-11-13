use torrant::{metainfo::Metainfo, tracker::Tracker};

#[tokio::main]
async fn main() {
    let x = std::fs::read("data/ubuntu.torrent").unwrap();
    let torrent = serde_bencode::from_bytes::<Metainfo>(&x).unwrap();

    let tracker_url = torrent.announce_url();

    let tracker = Tracker::new(tracker_url.to_string());
    let response = tracker.announce(torrent.info()).await.unwrap();

    println!("{:?}", response);
}
