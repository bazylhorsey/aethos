/// Constant-time byte slice comparison to prevent timing attacks on secrets.
///
/// Returns `true` iff `a == b` in time proportional to `a.len()`, not to
/// where the first differing byte is.
#[inline]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
