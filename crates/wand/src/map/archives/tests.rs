use std::{io::Write as _, net::TcpListener, thread};

use flate2::{Compression as Level, write::GzEncoder};

use super::*;
use crate::map::pmtiles::{header_bytes, varints};

const UUID: &str = "00000000-0000-4000-8000-000000000001";

#[test]
fn only_the_maps_cdn_s_archives_get_a_template_and_only_its_tiles_are_read() {
    let url = format!("{CDN}{}{FILE}", UUID.to_uppercase());
    let template = template(&url).unwrap();
    assert_eq!(template, format!("wand:pmtiles/{UUID}/{{z}}/{{x}}/{{y}}"));
    let tile = template.replace("{z}", "3").replace("{x}", "5").replace("{y}", "2");
    assert_eq!(parts(&tile), Some((Uuid::try_parse(UUID).unwrap(), 3, 5, 2)));

    for foreign in [
        "https://example.com/maps/x/map.pmtiles",
        &format!("{CDN}../{UUID}{FILE}"),
        &format!("http://maps-cdn.wemod.com/maps/{UUID}{FILE}"),
    ] {
        assert_eq!(super::template(foreign), None, "{foreign}");
    }
    for foreign in [format!("other:{PREFIX}{UUID}/0/0/0"), format!("wand:{PREFIX}{UUID}/0/0/0/1"), format!("wand:{PREFIX}..%2f/0/0/0")] {
        assert_eq!(Archives::default().tile(&foreign).unwrap_err().kind, FailureKind::Refused, "{foreign}");
    }
    let off_the_zoom = tile.replace("/3/5/2", "/1/4/0");
    assert_eq!(Archives::default().tile(&off_the_zoom).unwrap_err().kind, FailureKind::NotFound);
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Level::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

/// Tile 0 in the root; tiles 1 to 4 share bytes through a leaf; directories gzipped. The header says the data
/// section is `data` bytes long, of 10.
fn archive(data: u64) -> Vec<u8> {
    let leaf = gzip(&varints(&[1, 1, 4, 5, 6]));
    let root = gzip(&varints(&[2, 0, 1, 1, 0, 5, leaf.len() as u64, 1, 1]));
    let leaves = (127 + root.len() as u64, leaf.len() as u64);
    let mut bytes = header_bytes((127, root.len() as u64), leaves, (leaves.0 + leaves.1, data), 2, 1);
    bytes.extend([root, leaf, b"zero!tiles".to_vec()].concat());
    bytes
}

/// Answers every range of `file`, with the tag `etag` holds as each request comes.
fn serve(file: Vec<u8>, etag: Arc<Mutex<&'static str>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/map.pmtiles", listener.local_addr().unwrap());
    thread::spawn(move || {
        for mut stream in listener.incoming().map_while(Result::ok) {
            let mut asked = [0; 2048];
            let read = stream.read(&mut asked).unwrap();
            let asked = String::from_utf8_lossy(&asked[..read]).to_lowercase();
            let range = asked.split("range: bytes=").nth(1).and_then(|rest| rest.lines().next()).unwrap();
            let (from, last) = range.split_once('-').map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap())).unwrap();
            let body = &file[from.min(file.len())..(last + 1).min(file.len())];
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{}/{}\r\nContent-Length: {}\r\nETag: \"{}\"\r\nConnection: close\r\n\r\n",
                from + body.len() - 1,
                file.len(),
                body.len(),
                lock(&etag)
            );
            stream.write_all(&[head.as_bytes(), body].concat()).unwrap();
        }
    });
    url
}

#[test]
fn reads_root_and_leaf_tiles_and_detects_a_changed_etag() {
    let etag = Arc::new(Mutex::new("a"));
    let archive = Archive::open(serve(archive(10), etag.clone())).unwrap();
    assert_eq!(archive.tile(0).unwrap(), b"zero!");
    assert_eq!(archive.tile(3).unwrap(), b"tiles");
    assert_eq!(lock(&archive.leaves).len(), 1);
    assert_eq!(Failure::from(archive.tile(9).unwrap_err()).kind, FailureKind::NotFound);

    *lock(&etag) = "b";
    assert!(archive.tile(0).unwrap_err().downcast_ref::<Replaced>().is_some());
}

#[test]
fn an_entry_past_its_section_is_refused_unasked() {
    let archive = Archive::open(serve(archive(5), Arc::new(Mutex::new("a")))).unwrap();
    assert_eq!(archive.tile(0).unwrap(), b"zero!");
    assert_eq!(Failure::from(archive.tile(3).unwrap_err()).kind, FailureKind::Refused);
}

/// Needs the network and `IMPULSE_PMTILES`, an archive's address on the maps CDN, from a map's `tileSource.url`.
#[test]
#[ignore]
fn a_tile_comes_from_the_maps_cdn() {
    let template = template(&std::env::var("IMPULSE_PMTILES").unwrap()).unwrap();
    let tile = Archives::default().tile(&template.replace("{z}", "0").replace("{x}", "0").replace("{y}", "0")).unwrap();
    assert!(tile.starts_with(b"RIFF") && tile[8..12] == *b"WEBP", "a WebP tile");
}
