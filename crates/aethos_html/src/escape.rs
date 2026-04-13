/// Escape a string for safe insertion into HTML content or attribute values.
///
/// Replaces `&`, `<`, `>`, `"`, and `'` with their HTML entities.
/// Uses a byte scan so that ASCII-clean strings are handled without allocation.
pub fn html_escape(s: &str) -> String {
    // Fast path: scan bytes for any character that needs escaping.
    let needs_escape = s.bytes().any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\''));
    if !needs_escape {
        return s.to_owned();
    }

    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '&'  => out.push_str("&amp;"),
            '<'  => out.push_str("&lt;"),
            '>'  => out.push_str("&gt;"),
            '"'  => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _    => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_special_chars() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"hello\""), "&quot;hello&quot;");
    }
}
