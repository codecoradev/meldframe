//! The integers of the schema: Postgres has no unsigned columns, so counts and sequence numbers are `bigint` or
//! `integer`, and these functions are the one place a Rust number becomes a column and back.

/// A number as a `bigint` column holds it (a sequence number, a time in milliseconds, a limit); anything above
/// `i64::MAX` cannot be in the log, so it is clamped and never wraps.
pub(crate) fn db_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A number as an `integer` column holds it; clamped as [`db_i64`] is.
pub(crate) fn db_i32(n: u32) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// A `bigint` column as a number; a negative value (which no writer makes) reads as 0.
pub(crate) fn from_db_seq(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{db_i32, db_i64, from_db_seq};

    #[test]
    fn a_number_past_the_column_is_clamped_and_never_wraps() {
        assert_eq!(db_i64(0), 0);
        assert_eq!(db_i64(7), 7);
        assert_eq!(db_i64(i64::MAX as u64), i64::MAX);
        assert_eq!(db_i64(i64::MAX as u64 + 1), i64::MAX);
        assert_eq!(db_i64(u64::MAX), i64::MAX);
        assert_eq!(db_i32(i32::MAX as u32), i32::MAX);
        assert_eq!(db_i32(u32::MAX), i32::MAX);
    }

    #[test]
    fn a_negative_column_reads_as_zero() {
        assert_eq!(from_db_seq(0), 0);
        assert_eq!(from_db_seq(i64::MAX), i64::MAX as u64);
        assert_eq!(from_db_seq(-1), 0);
        assert_eq!(from_db_seq(i64::MIN), 0);
    }
}
