//! Properties of the fractional index used for sibling order (`Node.pos`): keys come out strictly
//! between their neighbours, stay valid, and grow slowly.

use meldframe_order::fracindex::{FracError, between, is_valid, n_between};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

#[test]
fn known_vectors() {
    // Vectors from the reference base-62 implementation (rocicorp/fractional-indexing).
    assert_eq!(between(None, None).unwrap(), "a0");
    assert_eq!(between(Some("a0"), None).unwrap(), "a1");
    assert_eq!(between(Some("a1"), None).unwrap(), "a2");
    assert_eq!(between(Some("a0"), Some("a1")).unwrap(), "a0V");
    assert_eq!(between(Some("a1"), Some("a2")).unwrap(), "a1V");
    assert_eq!(between(None, Some("a0")).unwrap(), "Zz");
    assert_eq!(between(None, Some("Zz")).unwrap(), "Zy");
    assert_eq!(between(Some("a0"), Some("a0V")).unwrap(), "a0G");
    assert_eq!(between(Some("a0V"), Some("a1")).unwrap(), "a0l");
    assert_eq!(between(Some("Zz"), Some("a0")).unwrap(), "ZzV");
    assert_eq!(between(Some("Zz"), Some("a1")).unwrap(), "a0");
    assert_eq!(between(Some("az"), None).unwrap(), "b00");
    assert_eq!(between(Some("Zz"), None).unwrap(), "a0");
    assert_eq!(
        n_between(None, None, 5).unwrap(),
        ["a0", "a1", "a2", "a3", "a4"]
    );
}

#[test]
fn rejects_invalid_and_misordered_keys() {
    assert!(matches!(
        between(Some("a1"), Some("a0")),
        Err(FracError::Order(..))
    ));
    assert!(matches!(
        between(Some("a1"), Some("a1")),
        Err(FracError::Order(..))
    ));
    let bad_keys = [
        "",
        "a",
        "a10",
        "!!",
        "a0 ",
        "0a",
        "A00000000000000000000000000",
        "é0",
    ];
    for bad in bad_keys {
        assert!(!is_valid(bad), "{bad:?}");
        assert!(matches!(
            between(Some(bad), None),
            Err(FracError::Invalid(_))
        ));
        assert!(matches!(
            between(None, Some(bad)),
            Err(FracError::Invalid(_))
        ));
    }
    for ok in [
        "a0",
        "a1",
        "Zz",
        "a0V",
        "b00",
        "az",
        "A00000000000000000000000001",
    ] {
        assert!(is_valid(ok), "{ok:?}");
    }
}

#[test]
fn between_is_strictly_between_for_random_insertions() {
    let mut rng = Rng(0xC0FFEE);
    let mut max_len = 0;
    for _round in 0..400 {
        // A sorted list that grows by inserting at a random gap, edges included.
        let mut keys: Vec<String> = Vec::new();
        for _ in 0..300 {
            let gap = rng.below(keys.len() + 1);
            let lo = (gap > 0).then(|| keys[gap - 1].as_str());
            let hi = keys.get(gap).map(String::as_str);
            let key = between(lo, hi).unwrap();
            assert!(is_valid(&key), "{key}");
            if let Some(lo) = lo {
                assert!(lo < key.as_str(), "{lo} < {key}");
            }
            if let Some(hi) = hi {
                assert!(key.as_str() < hi, "{key} < {hi}");
            }
            max_len = max_len.max(key.len());
            keys.insert(gap, key);
        }
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
    }
    assert!(max_len < 40, "longest key {max_len}");
}

#[test]
fn n_between_is_sorted_and_compact() {
    for n in [0usize, 1, 2, 3, 10, 62, 63, 500, 5000] {
        let keys = n_between(None, None, n).unwrap();
        assert_eq!(keys.len(), n);
        assert!(keys.windows(2).all(|w| w[0] < w[1]) && keys.iter().all(|k| is_valid(k)));
    }
    let keys = n_between(Some("a0"), Some("a1"), 100).unwrap();
    assert!(keys.windows(2).all(|w| w[0] < w[1]));
    assert!(keys[0].as_str() > "a0" && keys[99].as_str() < "a1");
}

#[test]
fn key_length_grows_slowly() {
    // Worst case for a gap: always insert right before the key that was just inserted.
    let (lo, mut hi) = ("a0".to_string(), "a1".to_string());
    for _ in 0..600 {
        let key = between(Some(&lo), Some(&hi)).unwrap();
        assert!(lo < key && key < hi);
        hi = key;
    }
    // About one character per six insertions into the same gap.
    assert!(hi.len() <= 120, "{}", hi.len());
    // Appending at the end grows logarithmically.
    let mut key = between(None, None).unwrap();
    for _ in 0..100_000 {
        key = between(Some(&key), None).unwrap();
    }
    assert!(key.len() <= 6, "{key}");
}
