//! `BlobStore::list`: the assets a workspace holds, in a stable order (sorted by id; the store keeps no upload
//! time or name), a page at a time with a cursor. One contract, run on every implementation.

use assets::{AssetError, BlobStore, DiskBlobs, ImageType, MAX_LIST, MemoryBlobs, asset_id};

/// A png whose tail makes it unique.
fn png(n: usize) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
    v.extend_from_slice(format!("image {n}").as_bytes());
    v
}

fn ids(
    store: &dyn BlobStore,
    ws: &str,
    after: Option<&str>,
    limit: usize,
) -> (Vec<String>, Option<String>) {
    let page = store.list(ws, after, limit).unwrap();
    (page.items.into_iter().map(|i| i.id).collect(), page.next)
}

fn contract(store: &dyn BlobStore) {
    // Nothing yet: an empty page, no cursor.
    let none = store.list("ws_1", None, 10).unwrap();
    assert!(none.items.is_empty() && none.next.is_none());

    let mut want: Vec<String> = (0..7)
        .map(|n| store.put("ws_1", &png(n)).unwrap().id)
        .collect();
    want.sort();
    store.put("ws_2", &png(100)).unwrap();
    store
        .put("ws_1", b"GIF89a\x01\0\x01\0 pixels")
        .map(|s| want.push(s.id))
        .unwrap();
    want.sort();

    // One page holds all of them, sorted by id, each with its size and its type told by the bytes.
    let all = store.list("ws_1", None, MAX_LIST).unwrap();
    assert!(all.next.is_none());
    assert_eq!(
        all.items.iter().map(|i| i.id.clone()).collect::<Vec<_>>(),
        want
    );
    let first = &all.items[0];
    let (kind, bytes) = store.get("ws_1", &first.id).unwrap();
    assert_eq!((first.kind, first.bytes), (kind, bytes.len()));
    assert!(all.items.iter().any(|i| i.kind == ImageType::Gif));

    // Pages of three walk the same list, each cursor is the last id of the page before, and the last page has none.
    let mut walked = Vec::new();
    let mut after: Option<String> = None;
    let mut pages = 0;
    loop {
        let (page, next) = ids(store, "ws_1", after.as_deref(), 3);
        assert!(page.len() <= 3);
        walked.extend(page.clone());
        pages += 1;
        match next {
            Some(n) => {
                assert_eq!(
                    Some(&n),
                    page.last(),
                    "the cursor is the last id of the page"
                );
                assert_eq!(
                    page.len(),
                    3,
                    "a cursor means the page is full and more follow"
                );
                after = Some(n);
            }
            None => break,
        }
    }
    assert_eq!((walked, pages), (want.clone(), 3));

    // A page that ends exactly at the end has no cursor: eight assets, pages of four.
    let (page, next) = ids(store, "ws_1", None, 4);
    assert_eq!(page.len(), 4);
    let (rest, end) = ids(store, "ws_1", next.as_deref(), 4);
    assert_eq!((rest.len(), end), (4, None));

    // An asset added between two calls shows up where its id sorts, and nothing is repeated or lost.
    let late = store.put("ws_1", &png(50)).unwrap().id;
    let (again, _) = ids(store, "ws_1", None, MAX_LIST);
    assert!(again.contains(&late) && again.windows(2).all(|w| w[0] < w[1]));

    // A limit past the cap is the cap; a limit of 0 is one asset at least, never an empty page with a cursor.
    let (many, _) = ids(store, "ws_1", None, usize::MAX);
    assert_eq!(many.len(), again.len());
    let (one, next) = ids(store, "ws_1", None, 0);
    assert_eq!(one.len(), 1);
    assert_eq!(next.as_ref(), one.first());

    // A cursor that names no asset still means "after this id".
    let (tail, _) = ids(store, "ws_1", Some(&"f".repeat(64)), 10);
    assert!(tail.is_empty());
    let (head, _) = ids(store, "ws_1", Some(&"0".repeat(64)), 100);
    assert_eq!(head, again);

    // Another workspace lists its own, and one that holds nothing lists nothing.
    assert_eq!(ids(store, "ws_2", None, 10).0, vec![asset_id(&png(100))]);
    assert!(ids(store, "ws_3", None, 10).0.is_empty());

    // A cursor that is not an id never reaches the store's paths.
    for bad in ["../../etc", "ABC", "", &"g".repeat(64)] {
        assert_eq!(
            store.list("ws_1", Some(bad), 10).unwrap_err(),
            AssetError::Invalid("an asset id is a lowercase hex SHA-256 (64 characters)".into()),
            "{bad:?}"
        );
    }
}

#[test]
fn memory_blobs_list_by_id_a_page_at_a_time() {
    contract(&MemoryBlobs::default());
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "meldframe-assets-list-{name}-{}-{}",
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
fn disk_blobs_list_by_id_a_page_at_a_time() {
    let root = scratch("contract");
    contract(&DiskBlobs::new(&root));
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn the_disk_walk_skips_what_is_not_an_asset() {
    let root = scratch("stray");
    let disk = DiskBlobs::new(&root);
    let kept = disk.put("ws_1", &png(1)).unwrap().id;
    let dir = disk.path_of("ws_1", &kept).unwrap();
    let shard = dir.parent().unwrap().to_path_buf();
    // A half-written file (a dot name), a file whose name is no id, a file that is no image under an id's name, and a
    // stray file beside the shard directories are not listed.
    std::fs::write(shard.join(format!(".{kept}.1.1.tmp")), b"half").unwrap();
    std::fs::write(shard.join("notes.txt"), b"not an asset").unwrap();
    let not_image = format!("{}{}", &kept[..2], "0".repeat(62));
    std::fs::write(shard.join(&not_image), b"plain text, not an image").unwrap();
    std::fs::write(shard.parent().unwrap().join("stray.txt"), b"x").unwrap();
    std::fs::create_dir_all(shard.parent().unwrap().join("zz")).unwrap();
    assert_eq!(ids(&disk, "ws_1", None, 10).0, vec![kept]);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_store_that_fails_says_unavailable() {
    // A root that is a file, not a directory: the walk cannot read it.
    let root = scratch("broken");
    let file = root.join("not-a-dir");
    std::fs::write(&file, b"x").unwrap();
    let disk = DiskBlobs::new(&file);
    let err = disk.list("ws_1", None, 10).unwrap_err();
    assert_eq!(err.code(), "unavailable");
    assert!(err.retryable());
    std::fs::remove_dir_all(&root).unwrap();
}

/// Several assets in one shard directory (ids that share their first two characters), made by hand: a real upload makes
/// such ids only by chance, and a walk that skipped the rest of the shard a cursor points into would lose them.
fn one_shard(disk: &DiskBlobs, ws: &str, n: usize) -> Vec<String> {
    let ids: Vec<String> = (0..n)
        .map(|i| format!("ab{i:02x}{}", "0".repeat(60)))
        .collect();
    for id in &ids {
        let path = disk.path_of(ws, id).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, png(7)).unwrap();
    }
    ids
}

#[test]
fn a_walk_that_ends_inside_a_shard_goes_on_in_that_shard() {
    let root = scratch("shard");
    let disk = DiskBlobs::new(&root);
    let mut want = one_shard(&disk, "ws_1", 5);
    // A shard before and one after, so the walk crosses from one to the next as well.
    let other = disk.put("ws_1", &png(900)).unwrap().id;
    want.push(other);
    want.sort();
    let mut walked = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..want.len() + 1 {
        let (page, next) = ids(&disk, "ws_1", after.as_deref(), 1);
        assert_eq!(page.len(), 1, "after {after:?}");
        walked.extend(page);
        match next {
            Some(n) => after = Some(n),
            None => break,
        }
    }
    assert_eq!(walked, want);
    // A cursor in the middle of the shard gives the rest of it, and one for an id that is not there too.
    let (rest, _) = ids(&disk, "ws_1", Some(&want[1]), 10);
    assert_eq!(rest, want[2..].to_vec());
    let between = format!("ab01{}", "1".repeat(60));
    let (rest, _) = ids(&disk, "ws_1", Some(&between), 10);
    assert!(
        rest.iter().all(|i| i.as_str() > between.as_str()) && rest.len() >= 3,
        "{rest:?}"
    );
    std::fs::remove_dir_all(&root).unwrap();
}
