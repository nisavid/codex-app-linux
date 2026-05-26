//! Redaction helpers for updater state and long-lived command logs.

use reqwest::Url;

pub(crate) const REDACTION_PLACEHOLDER: &str = "[REDACTED]";

pub(crate) fn redact_for_persistence(input: &str) -> String {
    redact_secret_assignments(&redact_urls(input))
}

pub(crate) fn redact_bytes_for_persistence(input: &[u8]) -> Vec<u8> {
    redact_for_persistence(&String::from_utf8_lossy(input)).into_bytes()
}

fn redact_urls(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some((url_start, scheme_len)) = next_url_start(&input[cursor..]) {
        let absolute_start = cursor + url_start;
        output.push_str(&input[cursor..absolute_start]);

        let token_start = absolute_start;
        let token_end = input[token_start..]
            .find(char::is_whitespace)
            .map(|offset| token_start + offset)
            .unwrap_or(input.len());
        let token = &input[token_start..token_end];
        let (url_candidate, trailing) = split_trailing_punctuation(token);
        if let Some(redacted) = redact_url_candidate(url_candidate, scheme_len) {
            output.push_str(&redacted);
        } else {
            output.push_str(url_candidate);
        }
        output.push_str(trailing);
        cursor = token_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn next_url_start(input: &str) -> Option<(usize, usize)> {
    input.char_indices().find_map(|(index, _)| {
        let candidate = &input[index..];
        if candidate
            .get(.."http://".len())
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("http://"))
        {
            Some((index, "http://".len()))
        } else if candidate
            .get(.."https://".len())
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
        {
            Some((index, "https://".len()))
        } else {
            None
        }
    })
}

fn quoted_value_end(input: &str, quote: u8, value_start: usize) -> usize {
    let bytes = input.as_bytes();
    let mut value_end = value_start + 1;
    while value_end < bytes.len() {
        match bytes[value_end] {
            b'\\' if value_end + 1 < bytes.len() => value_end += 2,
            byte if byte == quote => return value_end + 1,
            _ => value_end += 1,
        }
    }
    value_end
}

fn split_trailing_punctuation(token: &str) -> (&str, &str) {
    let mut end = token.len();
    while end > 0 {
        let Some(ch) = token[..end].chars().last() else {
            break;
        };
        if matches!(ch, '.' | ',' | ';' | ')' | ']' | '}' | '>' | '"' | '\'') {
            end -= ch.len_utf8();
        } else {
            break;
        }
    }
    (&token[..end], &token[end..])
}

fn redact_url_candidate(candidate: &str, _scheme_len: usize) -> Option<String> {
    let mut url = Url::parse(candidate).ok()?;
    let has_sensitive_parts = !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some();
    if !has_sensitive_parts {
        return None;
    }

    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

fn redact_secret_assignments(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    let mut index = 0;

    while index < bytes.len() {
        if !is_key_char(bytes[index]) {
            index += 1;
            continue;
        }

        let key_start = index;
        while index < bytes.len() && is_key_char(bytes[index]) {
            index += 1;
        }
        let key_end = index;
        let key = input[key_start..key_end].to_ascii_lowercase();
        if !is_secret_key(&key) {
            continue;
        }

        let mut separator = key_end;
        while separator < bytes.len() && matches!(bytes[separator], b' ' | b'\t') {
            separator += 1;
        }
        if separator >= bytes.len() || !matches!(bytes[separator], b'=' | b':') {
            continue;
        }

        let mut value_start = separator + 1;
        while value_start < bytes.len() && matches!(bytes[value_start], b' ' | b'\t') {
            value_start += 1;
        }
        if value_start >= bytes.len() {
            continue;
        }

        output.push_str(&input[cursor..value_start]);
        let value_end = redact_value(&mut output, input, &key, value_start);
        cursor = value_end;
        index = value_end;
    }

    output.push_str(&input[cursor..]);
    output
}

fn redact_value(output: &mut String, input: &str, key: &str, value_start: usize) -> usize {
    let bytes = input.as_bytes();
    match bytes[value_start] {
        quote @ (b'"' | b'\'') => {
            output.push(quote as char);
            output.push_str(REDACTION_PLACEHOLDER);
            let value_end = quoted_value_end(input, quote, value_start);
            if value_end <= bytes.len() && bytes.get(value_end - 1) == Some(&quote) {
                output.push(quote as char);
            }
            value_end
        }
        _ => {
            let value_end = unquoted_value_end(input, key, value_start);
            output.push_str(REDACTION_PLACEHOLDER);
            value_end
        }
    }
}

fn unquoted_value_end(input: &str, key: &str, value_start: usize) -> usize {
    let bytes = input.as_bytes();
    let mut value_end = value_start;
    while value_end < bytes.len() {
        let byte = bytes[value_end];
        let terminates = if is_authorization_key(key) {
            matches!(byte, b'\n' | b'\r' | b',' | b';')
        } else {
            byte.is_ascii_whitespace() || matches!(byte, b',' | b';')
        };
        if terminates {
            break;
        }
        value_end += 1;
    }
    value_end
}

fn is_key_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

fn is_secret_key(key: &str) -> bool {
    matches!(
        key,
        "token"
            | "access_token"
            | "refresh_token"
            | "password"
            | "passwd"
            | "secret"
            | "client_secret"
            | "api_key"
            | "apikey"
            | "_authtoken"
            | "authorization"
            | "proxy-authorization"
    )
}

fn is_authorization_key(key: &str) -> bool {
    matches!(key, "authorization" | "proxy-authorization")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_urls_before_persistence() {
        let input = "npm failed for <HTTPS://user:pass@example.com/pkg.tgz?token=secret#frag>";

        let redacted = redact_for_persistence(input);

        assert_eq!(redacted, "npm failed for <https://example.com/pkg.tgz>");
    }

    #[test]
    fn redacts_secret_assignments_before_persistence() {
        let input = "npm ERR! _authToken=shh access_token: abc123 api_key=\"quoted-\\\"secret\" package=codex-app";

        let redacted = redact_for_persistence(input);

        assert_eq!(
            redacted,
            "npm ERR! _authToken=[REDACTED] access_token: [REDACTED] api_key=\"[REDACTED]\" package=codex-app"
        );
    }

    #[test]
    fn redacts_authorization_payloads_before_persistence() {
        let input = "Authorization: Bearer abc.def.ghi\nok\nproxy authorization=Basic dXNlcjpwYXNz\nProxy-Authorization: Basic proxy-secret";

        let redacted = redact_for_persistence(input);

        assert_eq!(
            redacted,
            "Authorization: [REDACTED]\nok\nproxy authorization=[REDACTED]\nProxy-Authorization: [REDACTED]"
        );
        assert!(!redacted.contains("proxy-secret"));
    }

    #[test]
    fn preserves_ordinary_diagnostics() {
        let input =
            "installing /tmp/codex-app-26.513.31313-1-x86_64.pkg.tar.zst exited with status 1";

        let redacted = redact_for_persistence(input);

        assert_eq!(redacted, input);
    }
}
