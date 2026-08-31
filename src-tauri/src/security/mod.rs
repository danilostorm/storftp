use std::borrow::Cow;

const SECRET_MARKERS: [&str; 8] = [
    "password", "passphrase", "access_token", "refresh_token", "authorization", "client_secret", "private_key", "bearer ",
];

pub fn redact(input: &str) -> Cow<'_, str> {
    let lowercase = input.to_ascii_lowercase();
    if !SECRET_MARKERS.iter().any(|marker| lowercase.contains(marker)) {
        return Cow::Borrowed(input);
    }
    let mut output = input.to_string();
    for marker in SECRET_MARKERS {
        let mut search_from = 0usize;
        loop {
            let lower = output.to_ascii_lowercase();
            let Some(relative) = lower[search_from..].find(marker) else { break; };
            let start = search_from + relative;
            let value_start = (start + marker.len()).min(output.len());
            let tail = &output[value_start..];
            let sep_len = tail.chars().take_while(|c| matches!(c, ':' | '=' | ' ' | '\t' | '"' | '\'' )).map(char::len_utf8).sum::<usize>();
            let content_start = (value_start + sep_len).min(output.len());
            let content_tail = &output[content_start..];
            let content_len = content_tail.chars().take_while(|c| !matches!(c, ',' | ';' | '\n' | '\r' | '"' | '\'' | ' ')).map(char::len_utf8).sum::<usize>();
            if content_len > 0 {
                output.replace_range(content_start..content_start + content_len, "[REDACTED]");
                search_from = content_start + "[REDACTED]".len();
            } else {
                search_from = value_start.max(start + 1);
            }
            if search_from >= output.len() { break; }
        }
    }
    Cow::Owned(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_secrets() {
        let line = "password=abc123 authorization: Bearer secret refresh_token=xyz";
        let redacted = redact(line);
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("xyz"));
    }

    #[test]
    fn leaves_normal_log_unchanged() {
        let line = "transfer completed file=movie.mkv";
        assert_eq!(redact(line), line);
    }
}
