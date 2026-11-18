//! E2.5: an upload is png, jpeg, webp or gif (told by its bytes, not its name; SVG refused), is named by the SHA-256 of
//! its bytes, and is kept per workspace on local disk behind the blob seam.

use assets::{
    AssetError, BlobStore, DiskBlobs, ImageType, MAX_ASSET_BYTES, MemoryBlobs, asset_id,
    check_upload,
};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const JPEG: &[u8] = b"\xff\xd8\xff\xe0\0\x10JFIF";
const GIF: &[u8] = b"GIF89a\x01\0\x01\0";
const WEBP: &[u8] = b"RIFF\x24\0\0\0WEBPVP8 ";

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meldframe-assets-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_type_comes_from_the_bytes() {
    for (bytes, kind, mime) in [
        (PNG, ImageType::Png, "image/png"),
        (JPEG, ImageType::Jpeg, "image/jpeg"),
        (GIF, ImageType::Gif, "image/gif"),
        (WEBP, ImageType::Webp, "image/webp"),
    ] {
        let (k, _) = check_upload(bytes).unwrap();
        assert_eq!((k, k.mime()), (kind, mime));
    }
    assert!(
        check_upload(b"GIF87a\x01\0").is_ok(),
        "the older GIF header too"
    );
}

#[test]
fn svg_and_unknown_bytes_are_refused_with_a_stable_code() {
    for bytes in [
        &b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"[..],
        b"<?xml version=\"1.0\"?><svg/>",
        b"\xef\xbb\xbf<svg/>",
        b"%PDF-1.7",
        b"RIFF\x24\0\0\0WAVEfmt ",
    ] {
        let err = check_upload(bytes).unwrap_err();
        assert_eq!(err, AssetError::Unsupported, "{bytes:?}");
        assert_eq!(err.code(), "asset_type_refused");
    }
    assert_eq!(check_upload(b"").unwrap_err().code(), "invalid");
}

#[test]
fn an_upload_over_the_limit_is_refused() {
    let mut big = PNG.to_vec();
    big.resize(MAX_ASSET_BYTES + 1, 0);
    let err = check_upload(&big).unwrap_err();
    assert_eq!(err.code(), "asset_too_large");
    assert!(!err.retryable());
}

#[test]
fn the_id_is_the_lowercase_hex_sha256_of_the_bytes() {
    // SHA-256 of the empty string and of "abc", from FIPS 180-2.
    assert_eq!(
        asset_id(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        asset_id(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let (_, id) = check_upload(PNG).unwrap();
    assert_eq!(id, asset_id(PNG));
}

fn round_trip(store: &dyn BlobStore) {
    let a = store.put("ws_1", PNG).unwrap();
    assert_eq!(
        (a.id.as_str(), a.kind, a.bytes),
        (asset_id(PNG).as_str(), ImageType::Png, PNG.len())
    );
    // The same bytes again are the same asset: nothing new is written.
    let again = store.put("ws_1", PNG).unwrap();
    assert_eq!(again, a);
    let (kind, bytes) = store.get("ws_1", &a.id).unwrap();
    assert_eq!((kind, bytes.as_slice()), (ImageType::Png, PNG));
    assert!(store.has("ws_1", &a.id).unwrap());
    // Another workspace does not see it, even with the id.
    assert_eq!(
        store.get("ws_2", &a.id).unwrap_err().code(),
        "asset_not_found"
    );
    assert!(!store.has("ws_2", &a.id).unwrap());
    // An id that is not a lowercase hex SHA-256 never reaches the store (no path can be built from it).
    for bad in [
        "../../etc/passwd",
        "ABC",
        "",
        &"a".repeat(63),
        &"g".repeat(64),
    ] {
        assert_eq!(
            store.get("ws_1", bad).unwrap_err().code(),
            "invalid",
            "{bad:?}"
        );
    }
    // SVG is refused at the seam too, not only by the transport.
    assert_eq!(
        store.put("ws_1", b"<svg/>").unwrap_err().code(),
        "asset_type_refused"
    );
}

#[test]
fn memory_blobs_keep_assets_per_workspace() {
    round_trip(&MemoryBlobs::default());
}

#[test]
fn disk_blobs_keep_assets_per_workspace_and_survive_a_new_store() {
    let root = scratch("disk");
    let store = DiskBlobs::new(&root);
    round_trip(&store);
    // Storing the same bytes again does not write the file again.
    let path = store.path_of("ws_1", &asset_id(PNG)).unwrap();
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    store.put("ws_1", PNG).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
    // A new store on the same directory (a restart) finds the asset.
    let fresh = DiskBlobs::new(&root);
    let (kind, bytes) = fresh.get("ws_1", &asset_id(PNG)).unwrap();
    assert_eq!((kind, bytes.as_slice()), (ImageType::Png, PNG));
    // A workspace name with a slash or dots stays inside the root.
    let s = DiskBlobs::new(&root);
    let a = s.put("../ws/..", GIF).unwrap();
    assert_eq!(s.get("../ws/..", &a.id).unwrap().1, GIF);
    for entry in std::fs::read_dir(&root).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(name.bytes().all(|b| b.is_ascii_hexdigit()), "{name}");
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_file_changed_on_disk_is_not_served_under_its_old_id() {
    let root = scratch("tamper");
    let store = DiskBlobs::new(&root);
    let a = store.put("ws_1", PNG).unwrap();
    let path = store.path_of("ws_1", &a.id).unwrap();
    std::fs::write(&path, JPEG).unwrap();
    let err = store.get("ws_1", &a.id).unwrap_err();
    assert_eq!(err.code(), "asset_corrupt");
    assert!(!err.retryable(), "reading it again cannot help");
    // Uploading the same bytes again repairs it (a crash can leave a short file under the final name too).
    std::fs::write(&path, b"").unwrap();
    store.put("ws_1", PNG).unwrap();
    assert_eq!(store.get("ws_1", &a.id).unwrap().1, PNG);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_workspace_name_of_any_length_or_bytes_gets_a_directory_of_its_own() {
    let root = scratch("names");
    let store = DiskBlobs::new(&root);
    // 128 bytes, the longest workspace name the server takes: twice that in hex is over the file-name limit.
    let long = "w".repeat(128);
    store.put(&long, PNG).unwrap();
    assert_eq!(store.get(&long, &asset_id(PNG)).unwrap().1, PNG);
    // The empty name and a NUL are two workspaces.
    store.put("", GIF).unwrap();
    assert_eq!(
        store.get("\0", &asset_id(GIF)).unwrap_err().code(),
        "asset_not_found"
    );
    std::fs::remove_dir_all(&root).unwrap();
}
