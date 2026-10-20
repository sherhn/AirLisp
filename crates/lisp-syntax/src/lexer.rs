use crate::dialect::Dialect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Whitespace,
    /// `; ...` to end of line.
    Comment,
    /// `#| ... |#` (nested).
    BlockComment,
    String,
    /// `#\a`, `#\Space`.
    Char,
    Number,
    /// `:foo`
    Keyword,
    Symbol,
    Open,
    Close,
    /// Quote-like reader prefixes: `'` `` ` `` `,` `,@` `#'` `#+` `#(` ...
    Prefix,
    UnterminatedString,
    UnterminatedBlockComment,
}

impl TokenKind {
    pub fn is_trivia(self) -> bool {
        matches!(self, TokenKind::Whitespace | TokenKind::Comment | TokenKind::BlockComment | TokenKind::UnterminatedBlockComment)
    }
    pub fn is_comment(self) -> bool {
        matches!(self, TokenKind::Comment | TokenKind::BlockComment | TokenKind::UnterminatedBlockComment)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        &src[self.start..self.end]
    }
    pub fn len(&self) -> usize {
        self.end - self.start
    }
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

fn is_ws(b: u8, d: Dialect) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c) || (d == Dialect::Clojure && b == b',')
}

fn is_terminator(b: u8, d: Dialect) -> bool {
    if is_ws(b, d) {
        return true;
    }
    match b {
        b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'"' | b';' => true,
        b'\'' | b'`' | b',' => true,
        _ => false,
    }
}

/// Byte length of the UTF-8 char starting at `i`.
fn char_len(src: &str, i: usize) -> usize {
    src[i..].chars().next().map_or(1, char::len_utf8)
}

fn scan_symbol(src: &str, mut i: usize, d: Dialect) -> usize {
    let b = src.as_bytes();
    let n = b.len();
    while i < n {
        let c = b[i];
        if c == b'|' && d != Dialect::Clojure {
            i += 1;
            while i < n && b[i] != b'|' {
                if b[i] == b'\\' && i + 1 < n {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(n);
            continue;
        }
        if c == b'\\' && d != Dialect::Clojure {
            i += 1;
            if i < n {
                i += char_len(src, i);
            }
            continue;
        }
        if is_terminator(c, d) {
            break;
        }
        i += 1;
    }
    i
}

/// Does the text look like a numeric literal in any supported dialect?
pub fn is_number(s: &str) -> bool {
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    if t.is_empty() {
        return false;
    }
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit());
    }
    // ratio
    if let Some((a, b)) = t.split_once('/') {
        return !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit());
    }
    let t = t.strip_suffix(['N', 'M']).unwrap_or(t);
    let mut chars = t.chars().peekable();
    let mut digits = 0;
    while chars.peek().is_some_and(char::is_ascii_digit) {
        chars.next();
        digits += 1;
    }
    let mut frac = 0;
    let mut dot = false;
    if chars.peek() == Some(&'.') {
        dot = true;
        chars.next();
        while chars.peek().is_some_and(char::is_ascii_digit) {
            chars.next();
            frac += 1;
        }
    }
    if digits == 0 && frac == 0 {
        return false;
    }
    if let Some(c) = chars.peek().copied()
        && matches!(c, 'e' | 'E' | 'f' | 'F' | 'd' | 'D' | 's' | 'S' | 'l' | 'L')
    {
        chars.next();
        if matches!(chars.peek(), Some('+') | Some('-')) {
            chars.next();
        }
        let mut e = 0;
        while chars.peek().is_some_and(char::is_ascii_digit) {
            chars.next();
            e += 1;
        }
        if e == 0 {
            return false;
        }
    } else if dot && digits == 0 && frac == 0 {
        return false;
    }
    chars.next().is_none()
}

/// Split `src` into tokens. The tokens cover the whole input without gaps.
pub fn lex(src: &str, d: Dialect) -> Vec<Token> {
    let b = src.as_bytes();
    let n = b.len();
    let mut i = 0;
    let mut out = Vec::with_capacity(n / 4 + 8);
    while i < n {
        let s = i;
        let c = b[i];
        let kind = if is_ws(c, d) {
            while i < n && is_ws(b[i], d) {
                i += 1;
            }
            TokenKind::Whitespace
        } else {
            match c {
                b';' => {
                    while i < n && b[i] != b'\n' {
                        i += 1;
                    }
                    TokenKind::Comment
                }
                b'"' => {
                    i += 1;
                    let mut closed = false;
                    while i < n {
                        match b[i] {
                            b'\\' => i += 2,
                            b'"' => {
                                i += 1;
                                closed = true;
                                break;
                            }
                            _ => i += 1,
                        }
                    }
                    i = i.min(n);
                    if closed { TokenKind::String } else { TokenKind::UnterminatedString }
                }
                b'(' | b'[' | b'{' => {
                    i += 1;
                    TokenKind::Open
                }
                b')' | b']' | b'}' => {
                    i += 1;
                    TokenKind::Close
                }
                b'\'' | b'`' => {
                    i += 1;
                    TokenKind::Prefix
                }
                b'@' | b'^' if d == Dialect::Clojure => {
                    i += 1;
                    TokenKind::Prefix
                }
                b'~' if d == Dialect::Clojure => {
                    i += 1;
                    if i < n && b[i] == b'@' {
                        i += 1;
                    }
                    TokenKind::Prefix
                }
                b',' => {
                    i += 1;
                    if i < n && b[i] == b'@' {
                        i += 1;
                    }
                    TokenKind::Prefix
                }
                b'\\' if d == Dialect::Clojure => {
                    i += 1;
                    if i < n {
                        i += char_len(src, i);
                    }
                    while i < n && !is_terminator(b[i], d) {
                        i += 1;
                    }
                    TokenKind::Char
                }
                b'#' => lex_hash(src, &mut i, d),
                b':' => {
                    i = scan_symbol(src, i + 1, d);
                    TokenKind::Keyword
                }
                _ => {
                    i = scan_symbol(src, i, d);
                    if i == s {
                        // Defensive: always make progress.
                        i += char_len(src, i);
                    }
                    if is_number(&src[s..i]) { TokenKind::Number } else { TokenKind::Symbol }
                }
            }
        };
        out.push(Token { kind, start: s, end: i });
    }
    out
}

fn lex_hash(src: &str, i: &mut usize, d: Dialect) -> TokenKind {
    let b = src.as_bytes();
    let n = b.len();
    let s = *i;
    let next = b.get(s + 1).copied();
    match next {
        Some(b'|') if d.has_block_comments() => {
            let mut depth = 1;
            *i = s + 2;
            while *i < n {
                if b[*i] == b'|' && b.get(*i + 1) == Some(&b'#') {
                    depth -= 1;
                    *i += 2;
                    if depth == 0 {
                        return TokenKind::BlockComment;
                    }
                } else if b[*i] == b'#' && b.get(*i + 1) == Some(&b'|') {
                    depth += 1;
                    *i += 2;
                } else {
                    *i += 1;
                }
            }
            TokenKind::UnterminatedBlockComment
        }
        Some(b'\\') => {
            *i = s + 2;
            if *i < n {
                *i += char_len(src, *i);
            }
            while *i < n && !is_terminator(b[*i], d) {
                *i += 1;
            }
            TokenKind::Char
        }
        Some(b'\'' | b'.' | b'+' | b'-' | b'_' | b'(' | b'[' | b'{' | b'"' | b'?' | b'^' | b'=') => {
            // `#'f` `#.x` `#+feat` `#-feat` `#(` `#{` `#"re"` `#_`
            *i = s + 2;
            if let Some(b'(' | b'[' | b'{' | b'"') = next {
                *i = s + 1;
            }
            TokenKind::Prefix
        }
        Some(b';') if d == Dialect::Scheme || d == Dialect::Clojure => {
            *i = s + 2;
            TokenKind::Prefix
        }
        Some(b':') => {
            *i = scan_symbol(src, s + 2, d);
            TokenKind::Symbol
        }
        Some(c) if c.is_ascii_alphanumeric() => {
            *i = scan_symbol(src, s + 1, d);
            let t = &src[s..*i];
            let radix = t.len() > 2 && matches!(t.as_bytes()[1], b'x' | b'X' | b'b' | b'B' | b'o' | b'O' | b'e' | b'i' | b'd')
                && t[2..].bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'/' | b'-' | b'+'));
            if radix || t[1..].bytes().all(|c| c.is_ascii_digit()) && t.len() > 1 && src[*i..].starts_with(['r', 'R']) {
                TokenKind::Number
            } else {
                TokenKind::Symbol
            }
        }
        _ => {
            *i = scan_symbol(src, s + 1, d);
            if *i == s + 1 {
                // lone '#'
                return TokenKind::Prefix;
            }
            TokenKind::Symbol
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(s: &str) -> Vec<(TokenKind, &str)> {
        lex(s, Dialect::CommonLisp)
            .into_iter()
            .filter(|t| t.kind != TokenKind::Whitespace)
            .map(|t| (t.kind, &s[t.start..t.end]))
            .collect()
    }

    #[test]
    fn basic_list() {
        use TokenKind::*;
        assert_eq!(
            kinds("(defun foo (x) \"hi\" 42 :kw 'a)"),
            vec![
                (Open, "("),
                (Symbol, "defun"),
                (Symbol, "foo"),
                (Open, "("),
                (Symbol, "x"),
                (Close, ")"),
                (String, "\"hi\""),
                (Number, "42"),
                (Keyword, ":kw"),
                (Prefix, "'"),
                (Symbol, "a"),
                (Close, ")"),
            ]
        );
    }

    #[test]
    fn comments_and_blocks() {
        use TokenKind::*;
        assert_eq!(kinds("; hi\n#| a #| b |# c |# x"), vec![(Comment, "; hi"), (BlockComment, "#| a #| b |# c |#"), (Symbol, "x")]);
        assert_eq!(kinds("#| open")[0].0, UnterminatedBlockComment);
    }

    #[test]
    fn strings_with_escapes() {
        use TokenKind::*;
        assert_eq!(kinds(r#""a\"b" x"#), vec![(String, r#""a\"b""#), (Symbol, "x")]);
        assert_eq!(kinds("\"abc")[0].0, UnterminatedString);
    }

    #[test]
    fn chars_and_reader_macros() {
        use TokenKind::*;
        assert_eq!(kinds(r"#\Space #\( #'car"), vec![(Char, r"#\Space"), (Char, r"#\("), (Prefix, "#'"), (Symbol, "car")]);
        assert_eq!(kinds("#+sbcl x")[0], (Prefix, "#+"));
        assert_eq!(kinds("#(1 2)")[0], (Prefix, "#"));
        assert_eq!(kinds("#:foo")[0], (Symbol, "#:foo"));
        assert_eq!(kinds("#xFF")[0], (Number, "#xFF"));
        assert_eq!(kinds(",@x")[0], (Prefix, ",@"));
    }

    #[test]
    fn numbers() {
        for ok in ["1", "-1", "+1", "1.5", ".5", "1.", "1e10", "1.5d0", "1/2", "-3/4", "0xFF", "10N"] {
            assert!(is_number(ok), "{ok}");
        }
        for bad in ["-", "+", "1+", "1-", "foo", "e5", ".", "1/", "/2", "1e", "*x*"] {
            assert!(!is_number(bad), "{bad}");
        }
    }

    #[test]
    fn symbols_with_bars_and_escapes() {
        use TokenKind::*;
        assert_eq!(kinds("|a b| c"), vec![(Symbol, "|a b|"), (Symbol, "c")]);
        assert_eq!(kinds(r"a\ b c"), vec![(Symbol, r"a\ b"), (Symbol, "c")]);
    }

    #[test]
    fn tokens_cover_input_without_gaps() {
        let s = "(defun é (x) ; ünï\n  #\\é \"日本\" #| ж |# 'x)";
        let toks = lex(s, Dialect::CommonLisp);
        let mut pos = 0;
        for t in &toks {
            assert_eq!(t.start, pos);
            pos = t.end;
        }
        assert_eq!(pos, s.len());
    }

    #[test]
    fn clojure_specifics() {
        let s = "(let [a 1, b 2] #{a} \\a ~x ~@y)";
        let toks: Vec<_> = lex(s, Dialect::Clojure).into_iter().filter(|t| t.kind != TokenKind::Whitespace).collect();
        assert!(toks.iter().any(|t| t.kind == TokenKind::Char && t.text(s) == "\\a"));
        assert!(toks.iter().any(|t| t.kind == TokenKind::Prefix && t.text(s) == "~@"));
    }

    #[test]
    fn never_panics_on_garbage() {
        for s in ["#", "#\\", "\\", "|", "\"\\", "#|", "'", ",@", "é", "#é", "a|b", "#xyz", "(#", "\u{0}"] {
            for d in [Dialect::CommonLisp, Dialect::Scheme, Dialect::Clojure, Dialect::EmacsLisp] {
                let toks = lex(s, d);
                assert_eq!(toks.last().map_or(0, |t| t.end), s.len());
            }
        }
    }
}
