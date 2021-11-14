use torrant::{metainfo::Metainfo, tracker::Tracker, Url};

#[tokio::main]
async fn main() {
    let x = std::fs::read("data/ubuntu.torrent").unwrap();
    let torrent = serde_bencode::from_bytes::<Metainfo>(&x).unwrap();

    let tracker_url = torrent.announce_url();

    let tracker = Tracker::new(Url::parse(tracker_url).expect("malformed announce url"));
    let response = tracker.announce(torrent.info()).await.unwrap();

    println!("{:?}", response);
    //println!("Found {} peers", response.peers.len());
}
