//! The one hash the tools make ids and digests with.

/// FNV-1a, 64 bits, over a stream of bytes: the one hash the tools make ids and digests with (it must not change, or
/// a retry of an old key would find other ids).
pub fn fnv1a(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}
