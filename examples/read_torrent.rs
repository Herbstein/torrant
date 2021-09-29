use torrant::metainfo::Metainfo;

fn main() {
    let x = std::fs::read("data/test.torrent").unwrap();
    let info = serde_bencode::from_bytes::<Metainfo>(&x).unwrap();

    println!("{:#?}", info);
}
