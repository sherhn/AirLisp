//! Wire format shared with `lisp/repl-server.lisp`.

pub const SOH: char = '\u{1}';

pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\\' => o.push_str("\\\\"),
            c => o.push(c),
        }
    }
    o
}

pub fn unescape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match it.next() {
            Some('n') => o.push('\n'),
            Some('r') => o.push('\r'),
            Some('t') => o.push('\t'),
            Some(d) => o.push(d),
            None => o.push('\\'),
        }
    }
    o
}

/// `kind \t id \t payload`
pub fn request_line(kind: &str, id: u64, payload: &str) -> String {
    format!("{kind}\t{id}\t{}\n", escape(payload))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub kind: String,
    pub id: u64,
    pub payload: String,
}

/// Parse a protocol line (without the trailing newline); `None` if it is not one.
pub fn parse_message(line: &str) -> Option<Message> {
    let rest = line.strip_prefix(SOH)?;
    let mut parts = rest.splitn(3, '\t');
    let kind = parts.next()?.to_string();
    let id = parts.next()?.parse().ok()?;
    let payload = unescape(parts.next().unwrap_or(""));
    Some(Message { kind, id, payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_roundtrip() {
        for s in ["", "plain", "a\nb", "tab\there", "back\\slash", "\\n literally", "mix \r\n \\ \t é日本", "trailing\\"] {
            assert_eq!(unescape(&escape(s)), s, "{s:?}");
        }
        assert!(!escape("a\nb\tc").contains('\n'));
        assert!(!escape("a\nb\tc").contains('\t'));
    }

    #[test]
    fn request_format() {
        assert_eq!(request_line("eval", 7, "(+ 1\n2)"), "eval\t7\t(+ 1\\n2)\n");
    }

    #[test]
    fn message_parsing() {
        let m = parse_message("\u{1}value\t12\t3\\n4").unwrap();
        assert_eq!(m, Message { kind: "value".into(), id: 12, payload: "3\n4".into() });
        assert!(parse_message("plain output").is_none());
        assert!(parse_message("\u{1}bad").is_none());
        assert!(parse_message("\u{1}x\tnotnum\tp").is_none());
        assert_eq!(parse_message("\u{1}done\t3\t").unwrap().payload, "");
    }
}
