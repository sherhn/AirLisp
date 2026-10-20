use crate::dialect::Dialect;
use crate::lexer::{Token, TokenKind, lex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub start: usize,
    pub end: usize,
    pub severity: Severity,
    pub message: String,
}

/// A fully lexed and bracket-matched document.
#[derive(Clone, Debug)]
pub struct Parsed {
    pub dialect: Dialect,
    pub tokens: Vec<Token>,
    /// For every Open/Close token: index of its partner token.
    pub partner: Vec<Option<usize>>,
    /// Nesting depth for every bracket token (0 = top level).
    pub depth: Vec<u32>,
    /// True for `#+` / `#-` prefix tokens, which consume two datums (feature expression and form).
    pub feature_prefix: Vec<bool>,
    pub diagnostics: Vec<Diagnostic>,
}

fn closer_for(open: u8) -> u8 {
    match open {
        b'(' => b')',
        b'[' => b']',
        _ => b'}',
    }
}

pub fn parse(src: &str, dialect: Dialect) -> Parsed {
    let tokens = lex(src, dialect);
    let mut partner = vec![None; tokens.len()];
    let mut depth = vec![0u32; tokens.len()];
    let mut diagnostics = Vec::new();
    let feature_prefix: Vec<bool> = tokens
        .iter()
        .map(|t| t.kind == TokenKind::Prefix && matches!(&src[t.start..t.end], "#+" | "#-"))
        .collect();
    let mut stack: Vec<usize> = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        match t.kind {
            TokenKind::Open => {
                depth[i] = stack.len() as u32;
                stack.push(i);
            }
            TokenKind::Close => {
                let c = src.as_bytes()[t.start];
                match stack.pop() {
                    Some(o) => {
                        depth[i] = stack.len() as u32;
                        partner[i] = Some(o);
                        partner[o] = Some(i);
                        let want = closer_for(src.as_bytes()[tokens[o].start]);
                        if want != c {
                            diagnostics.push(Diagnostic {
                                start: t.start,
                                end: t.end,
                                severity: Severity::Error,
                                message: format!("Mismatched bracket: expected '{}' but found '{}'", want as char, c as char),
                            });
                        }
                    }
                    None => diagnostics.push(Diagnostic {
                        start: t.start,
                        end: t.end,
                        severity: Severity::Error,
                        message: format!("Unmatched '{}'", c as char),
                    }),
                }
            }
            TokenKind::UnterminatedString => diagnostics.push(Diagnostic {
                start: t.start,
                end: t.end,
                severity: Severity::Error,
                message: "Unterminated string".into(),
            }),
            TokenKind::UnterminatedBlockComment => diagnostics.push(Diagnostic {
                start: t.start,
                end: t.end,
                severity: Severity::Error,
                message: "Unterminated block comment".into(),
            }),
            _ => {}
        }
    }
    for o in stack {
        let t = tokens[o];
        let want = closer_for(src.as_bytes()[t.start]);
        diagnostics.push(Diagnostic {
            start: t.start,
            end: t.end,
            severity: Severity::Error,
            message: format!("Unclosed '{}' (missing '{}')", src.as_bytes()[t.start] as char, want as char),
        });
    }
    diagnostics.sort_by_key(|d| d.start);
    Parsed { dialect, tokens, partner, depth, feature_prefix, diagnostics }
}

/// Inclusive token range of a datum (including its reader prefixes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Datum {
    pub first: usize,
    pub last: usize,
}

impl Parsed {
    pub fn start_of(&self, d: Datum) -> usize {
        self.tokens[d.first].start
    }
    pub fn end_of(&self, d: Datum) -> usize {
        self.tokens[d.last].end
    }

    /// Index of the first non-trivia token at or after `i`.
    pub fn skip_trivia(&self, mut i: usize) -> usize {
        while i < self.tokens.len() && self.tokens[i].kind.is_trivia() {
            i += 1;
        }
        i
    }

    /// Datum starting at token `i` (which must be non-trivia). Prefixes bind to the following datum.
    /// Returns `None` at a closing bracket or end of input.
    pub fn datum_at(&self, i: usize) -> Option<Datum> {
        let n = self.tokens.len();
        if i >= n {
            return None;
        }
        let mut j = i;
        loop {
            match self.tokens[j].kind {
                TokenKind::Prefix => {
                    let feature = self.feature_prefix[j];
                    j = self.skip_trivia(j + 1);
                    if j >= n {
                        return Some(Datum { first: i, last: n - 1 });
                    }
                    if feature {
                        // `#+feature form`: skip the feature expression, then the form follows.
                        match self.datum_at(j) {
                            Some(f) => j = self.skip_trivia(f.last + 1),
                            None => return Some(Datum { first: i, last: j - 1 }),
                        }
                        if j >= n {
                            return Some(Datum { first: i, last: n - 1 });
                        }
                    }
                    continue;
                }
                TokenKind::Close => {
                    return if j == i { None } else { Some(Datum { first: i, last: j - 1 }) };
                }
                TokenKind::Open => {
                    let last = self.partner[j].unwrap_or(n - 1);
                    return Some(Datum { first: i, last });
                }
                _ => return Some(Datum { first: i, last: j }),
            }
        }
    }

    /// Datums directly inside the list opened by token `open`.
    pub fn children(&self, open: usize) -> Vec<Datum> {
        let end = self.partner[open].unwrap_or(self.tokens.len());
        let mut out = Vec::new();
        let mut i = open + 1;
        while i < end {
            i = self.skip_trivia(i);
            if i >= end {
                break;
            }
            match self.datum_at(i) {
                Some(d) => {
                    i = d.last + 1;
                    out.push(d);
                }
                None => break,
            }
        }
        out
    }

    /// Top-level datums of the document.
    pub fn top_level(&self) -> Vec<Datum> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.tokens.len() {
            i = self.skip_trivia(i);
            if i >= self.tokens.len() {
                break;
            }
            match self.datum_at(i) {
                Some(d) => {
                    i = d.last + 1;
                    out.push(d);
                }
                None => i += 1, // stray closer
            }
        }
        out
    }

    /// Index of the token containing `offset` (the token whose range is `start <= offset < end`).
    pub fn token_index_at(&self, offset: usize) -> Option<usize> {
        let idx = self.tokens.partition_point(|t| t.end <= offset);
        (idx < self.tokens.len() && self.tokens[idx].start <= offset).then_some(idx)
    }

    /// Innermost `Open` token whose list contains `offset` strictly inside the brackets
    /// (i.e. `open.end <= offset <= close.start`).
    pub fn enclosing_open(&self, offset: usize) -> Option<usize> {
        let mut best = None;
        for (i, t) in self.tokens.iter().enumerate() {
            if t.start >= offset {
                break;
            }
            if t.kind == TokenKind::Open && t.end <= offset {
                match self.partner[i] {
                    Some(c) if self.tokens[c].start < offset => {}
                    _ => best = Some(i),
                }
            }
        }
        best
    }

    /// Is `offset` inside a string or a block comment (not at its first byte)?
    pub fn in_string_or_block_comment(&self, offset: usize) -> bool {
        self.token_index_at(offset).is_some_and(|i| {
            let t = self.tokens[i];
            matches!(t.kind, TokenKind::String | TokenKind::UnterminatedString | TokenKind::BlockComment | TokenKind::UnterminatedBlockComment)
                && offset > t.start
                && (offset < t.end || matches!(t.kind, TokenKind::UnterminatedString | TokenKind::UnterminatedBlockComment))
        })
    }

    pub fn in_line_comment(&self, offset: usize) -> bool {
        self.token_index_at(offset).is_some_and(|i| self.tokens[i].kind == TokenKind::Comment && offset > self.tokens[i].start)
    }

    /// Bracket matching as the IDE shows it: the bracket just before or at the caret.
    /// Returns (bracket_offset, partner_offset).
    pub fn matching_bracket(&self, caret: usize) -> Option<(usize, usize)> {
        let candidates = [caret.checked_sub(1), Some(caret)];
        for off in candidates.into_iter().flatten() {
            if let Some(i) = self.token_index_at(off)
                && let Some(p) = self.partner[i]
                && matches!(self.tokens[i].kind, TokenKind::Open | TokenKind::Close)
            {
                return Some((self.tokens[i].start, self.tokens[p].start));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Parsed {
        parse(s, Dialect::CommonLisp)
    }

    #[test]
    fn matches_brackets_and_depth() {
        let s = "(a (b) [c])";
        let r = p(s);
        assert!(r.diagnostics.is_empty());
        let opens: Vec<_> = r.tokens.iter().enumerate().filter(|(_, t)| t.kind == TokenKind::Open).map(|(i, _)| i).collect();
        assert_eq!(r.depth[opens[0]], 0);
        assert_eq!(r.depth[opens[1]], 1);
        assert_eq!(r.depth[opens[2]], 1);
        assert_eq!(r.matching_bracket(1), Some((0, 10)));
        assert_eq!(r.matching_bracket(11), Some((10, 0)));
    }

    #[test]
    fn reports_unbalanced() {
        let r = p("(a (b)");
        assert_eq!(r.diagnostics.len(), 1);
        assert!(r.diagnostics[0].message.starts_with("Unclosed"));
        let r = p("a)");
        assert!(r.diagnostics[0].message.contains("Unmatched"));
        let r = p("(a]");
        assert!(r.diagnostics[0].message.contains("Mismatched"));
        let r = p("\"abc");
        assert_eq!(r.diagnostics[0].message, "Unterminated string");
    }

    #[test]
    fn top_level_with_prefixes() {
        let s = "'a (b c) #+sbcl (d) ; x\n#(1 2)";
        let r = p(s);
        let tl: Vec<_> = r.top_level().iter().map(|d| &s[r.start_of(*d)..r.end_of(*d)]).collect();
        assert_eq!(tl, vec!["'a", "(b c)", "#+sbcl (d)", "#(1 2)"]);
    }

    #[test]
    fn children_of_list() {
        let s = "(f 'x (g) \"s\")";
        let r = p(s);
        let ch: Vec<_> = r.children(0).iter().map(|d| &s[r.start_of(*d)..r.end_of(*d)]).collect();
        assert_eq!(ch, vec!["f", "'x", "(g)", "\"s\""]);
    }

    #[test]
    fn enclosing_open_works() {
        let s = "(a (b c) d)";
        let r = p(s);
        assert_eq!(r.enclosing_open(0), None);
        assert_eq!(r.tokens[r.enclosing_open(1).unwrap()].start, 0);
        assert_eq!(r.tokens[r.enclosing_open(6).unwrap()].start, 3);
        assert_eq!(r.tokens[r.enclosing_open(7).unwrap()].start, 3); // right before ')'
        assert_eq!(r.tokens[r.enclosing_open(8).unwrap()].start, 0); // after ')'
        assert_eq!(r.enclosing_open(11), None);
    }

    #[test]
    fn string_context() {
        let r = p("(a \"bc\" ; x\n)");
        assert!(!r.in_string_or_block_comment(3));
        assert!(r.in_string_or_block_comment(4));
        assert!(!r.in_string_or_block_comment(7));
        assert!(r.in_line_comment(9));
    }
}
