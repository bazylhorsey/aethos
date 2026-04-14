/// Percent-encodes a string for use in a URL path segment.
///
/// Unreserved characters (`A-Z a-z 0-9 - _ . ~`) pass through unchanged.
/// Spaces become `%20`. All other bytes are encoded as `%XX`.
///
/// # Example
/// ```
/// use aethos_core::url_encode;
/// assert_eq!(url_encode("Hello World"), "Hello%20World");
/// assert_eq!(url_encode("café"), "caf%C3%A9");
/// ```
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push_str("%20"),
            other => {
                out.push('%');
                out.push(char::from_digit((other >> 4) as u32, 16).unwrap().to_ascii_uppercase());
                out.push(char::from_digit((other & 0xf) as u32, 16).unwrap().to_ascii_uppercase());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_unreserved() {
        assert_eq!(url_encode("hello-world_1.0~"), "hello-world_1.0~");
    }

    #[test]
    fn encode_space() {
        assert_eq!(url_encode("Hello World"), "Hello%20World");
    }

    #[test]
    fn encode_multibyte() {
        assert_eq!(url_encode("café"), "caf%C3%A9");
    }

    #[test]
    fn encode_special() {
        assert_eq!(url_encode("a/b?c=d&e"), "a%2Fb%3Fc%3Dd%26e");
    }
}
