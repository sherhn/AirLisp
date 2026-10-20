//! Structural (paredit style) editing and sexp navigation. All offsets are byte offsets.

use crate::lexer::TokenKind;
use crate::parse::{Datum, Parsed};

/// Result of a structural edit: the full new text and the new caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rewrite {
    pub text: String,
    pub caret: usize,
}

/// Siblings of the list datum that contains token `open` (top level when no parent).
fn siblings_of(p: &Parsed, open: usize, src_len_hint: usize) -> Vec<Datum> {
    let _ = src_len_hint;
    let start = p.tokens[open].start;
    match p.enclosing_open(start) {
        Some(parent) => p.children(parent),
        None => p.top_level(),
    }
}

/// The datum "at" the caret: the one containing it, starting at it, ending at it, or the next one.
pub fn datum_at_caret(p: &Parsed, caret: usize) -> Option<Datum> {
    let siblings_at = |off: usize| -> Vec<Datum> {
        match p.enclosing_open(off) {
            Some(o) => p.children(o),
            None => p.top_level(),
        }
    };
    let sibs = siblings_at(caret);
    // strictly inside / starting at
    for d in &sibs {
        let (s, e) = (p.start_of(*d), p.end_of(*d));
        if s < caret && caret < e && !matches!(p.tokens[d.first].kind, TokenKind::Open) {
            return Some(*d);
        }
        if s == caret {
            return Some(*d);
        }
    }
    // caret directly after a datum
    for d in &sibs {
        if p.end_of(*d) == caret {
            return Some(*d);
        }
    }
    // inside a list datum (not at its bracket): descend handled by sibs of enclosing; otherwise next
    sibs.into_iter().find(|d| p.start_of(*d) >= caret)
}

pub fn forward_sexp(p: &Parsed, caret: usize) -> Option<usize> {
    let sibs = match p.enclosing_open(caret) {
        Some(o) => p.children(o),
        None => p.top_level(),
    };
    sibs.into_iter().find(|d| p.end_of(*d) > caret).map(|d| p.end_of(d))
}

pub fn backward_sexp(p: &Parsed, caret: usize) -> Option<usize> {
    let sibs = match p.enclosing_open(caret) {
        Some(o) => p.children(o),
        None => p.top_level(),
    };
    sibs.into_iter().rev().find(|d| p.start_of(*d) < caret).map(|d| p.start_of(d))
}

/// Move to the opening bracket of the enclosing list.
pub fn up_list(p: &Parsed, caret: usize) -> Option<usize> {
    p.enclosing_open(caret).map(|o| p.tokens[o].start)
}

/// Move just inside the next list opening after the caret.
pub fn down_list(p: &Parsed, caret: usize) -> Option<usize> {
    p.tokens.iter().find(|t| t.kind == TokenKind::Open && t.start >= caret).map(|t| t.end)
}

/// Top-level form containing the caret, or the closest preceding one.
pub fn toplevel_form_at(p: &Parsed, caret: usize) -> Option<(usize, usize)> {
    let tl = p.top_level();
    let mut best = None;
    for d in &tl {
        let (s, e) = (p.start_of(*d), p.end_of(*d));
        if s <= caret && caret <= e {
            return Some((s, e));
        }
        if e < caret {
            best = Some((s, e));
        }
    }
    best.or_else(|| tl.first().map(|d| (p.start_of(*d), p.end_of(*d))))
}

/// The sexp that ends at (or just before) the caret.
pub fn sexp_before(src: &str, p: &Parsed, caret: usize) -> Option<(usize, usize)> {
    let sibs = match p.enclosing_open(caret) {
        Some(o) => p.children(o),
        None => p.top_level(),
    };
    let d = sibs.into_iter().rev().find(|d| p.start_of(*d) < caret.min(p.end_of(*d)) || p.end_of(*d) <= caret)?;
    let (s, e) = (p.start_of(d), p.end_of(d));
    // if the caret is inside the datum, this is the enclosing atom/list up to the caret only when complete
    if e > caret && src[caret..e].trim().is_empty() {
        return Some((s, e));
    }
    (e <= caret).then_some((s, e))
}

pub fn forward_slurp(src: &str, p: &Parsed, caret: usize) -> Option<Rewrite> {
    let o = p.enclosing_open(caret)?;
    let c = p.partner[o]?;
    let close = p.tokens[c];
    let next = p.skip_trivia(c + 1);
    let d = p.datum_at(next)?;
    let end = p.end_of(d);
    let mut text = String::with_capacity(src.len());
    text.push_str(&src[..close.start]);
    text.push_str(&src[close.end..end]);
    text.push_str(&src[close.start..close.end]);
    text.push_str(&src[end..]);
    Some(Rewrite { text, caret })
}

pub fn forward_barf(src: &str, p: &Parsed, caret: usize) -> Option<Rewrite> {
    let o = p.enclosing_open(caret)?;
    let c = p.partner[o]?;
    let close = p.tokens[c];
    let kids = p.children(o);
    let last = *kids.last()?;
    let prev_end = if kids.len() >= 2 { p.end_of(kids[kids.len() - 2]) } else { p.tokens[o].end };
    let ls = p.start_of(last);
    let mut text = String::with_capacity(src.len());
    text.push_str(&src[..prev_end]);
    text.push_str(&src[close.start..close.end]);
    // whitespace between previous child and the barfed one; keep at least one space
    let ws = &src[prev_end..ls];
    text.push_str(if ws.is_empty() { " " } else { ws });
    text.push_str(&src[ls..close.start]);
    text.push_str(&src[close.end..]);
    let new_close = prev_end;
    Some(Rewrite { text, caret: caret.min(new_close) })
}

pub fn backward_slurp(src: &str, p: &Parsed, caret: usize) -> Option<Rewrite> {
    let o = p.enclosing_open(caret)?;
    let open = p.tokens[o];
    let sibs = siblings_of(p, o, src.len());
    let me = sibs.iter().position(|d| d.first <= o && o <= d.last)?;
    let prev = *sibs.get(me.checked_sub(1)?)?;
    let ps = p.start_of(prev);
    let mut text = String::with_capacity(src.len());
    text.push_str(&src[..ps]);
    text.push_str(&src[open.start..open.end]);
    text.push_str(&src[ps..open.start]);
    text.push_str(&src[open.end..]);
    Some(Rewrite { text, caret: caret.max(ps + 1) })
}

pub fn backward_barf(src: &str, p: &Parsed, caret: usize) -> Option<Rewrite> {
    let o = p.enclosing_open(caret)?;
    let open = p.tokens[o];
    let kids = p.children(o);
    let first = *kids.first()?;
    let c = p.partner[o]?;
    let next_start = kids.get(1).map(|d| p.start_of(*d)).unwrap_or(p.tokens[c].start);
    let fe = p.end_of(first);
    let ws = &src[fe..next_start];
    let mut text = String::with_capacity(src.len());
    text.push_str(&src[..open.start]);
    text.push_str(&src[p.start_of(first)..fe]);
    text.push_str(if ws.is_empty() { " " } else { ws });
    text.push_str(&src[open.start..open.end]);
    text.push_str(&src[next_start..]);
    Some(Rewrite { text, caret: caret.max(fe + 1) })
}

/// Replace the enclosing list by the sexp at the caret.
pub fn raise(src: &str, p: &Parsed, caret: usize) -> Option<Rewrite> {
    let d = datum_at_caret(p, caret)?;
    let o = p.enclosing_open(p.start_of(d))?;
    let c = p.partner[o]?;
    let sibs = siblings_of(p, o, src.len());
    let list = sibs.iter().find(|x| x.first <= o && c <= x.last)?;
    let (ls, le) = (p.start_of(*list), p.end_of(*list));
    let inner = &src[p.start_of(d)..p.end_of(d)];
    let mut text = String::with_capacity(src.len());
    text.push_str(&src[..ls]);
    text.push_str(inner);
    text.push_str(&src[le..]);
    Some(Rewrite { text, caret: ls })
}

/// Remove the brackets of the enclosing list, keeping its contents.
pub fn splice(src: &str, p: &Parsed, caret: usize) -> Option<Rewrite> {
    let o = p.enclosing_open(caret)?;
    let c = p.partner[o]?;
    let (open, close) = (p.tokens[o], p.tokens[c]);
    let mut text = String::with_capacity(src.len());
    text.push_str(&src[..open.start]);
    text.push_str(&src[open.end..close.start]);
    text.push_str(&src[close.end..]);
    Some(Rewrite { text, caret: caret.saturating_sub(1).max(open.start) })
}

/// Wrap the sexp at the caret in parentheses.
pub fn wrap_round(src: &str, p: &Parsed, caret: usize) -> Option<Rewrite> {
    let d = datum_at_caret(p, caret)?;
    let (s, e) = (p.start_of(d), p.end_of(d));
    let mut text = String::with_capacity(src.len() + 2);
    text.push_str(&src[..s]);
    text.push('(');
    text.push_str(&src[s..e]);
    text.push(')');
    text.push_str(&src[e..]);
    Some(Rewrite { text, caret: s + 1 })
}

/// Delete the sexp at the caret.
pub fn kill_sexp(src: &str, p: &Parsed, caret: usize) -> Option<Rewrite> {
    let d = datum_at_caret(p, caret)?;
    let (s, mut e) = (p.start_of(d), p.end_of(d));
    // swallow one run of following spaces on the same line
    while src[e..].starts_with(' ') {
        e += 1;
    }
    let mut text = String::with_capacity(src.len());
    text.push_str(&src[..s]);
    text.push_str(&src[e..]);
    Some(Rewrite { text, caret: s })
}

/// Grow a selection to the next syntactic level (token -> sexp -> list contents -> list ...).
pub fn extend_selection(src: &str, p: &Parsed, sel: (usize, usize)) -> (usize, usize) {
    let (s, e) = (sel.0.min(sel.1), sel.0.max(sel.1));
    let mut cands: Vec<(usize, usize)> = Vec::new();
    // tokens around the selection
    if let Some(i) = p.token_index_at(s) {
        let t = p.tokens[i];
        if !matches!(t.kind, TokenKind::Whitespace | TokenKind::Open | TokenKind::Close | TokenKind::Prefix) {
            cands.push((t.start, t.end));
            if matches!(t.kind, TokenKind::String) && t.len() >= 2 {
                cands.push((t.start + 1, t.end - 1));
            }
        }
    }
    if s > 0
        && s == e
        && let Some(i) = p.token_index_at(s - 1)
    {
        let t = p.tokens[i];
        if !matches!(t.kind, TokenKind::Whitespace | TokenKind::Open | TokenKind::Close | TokenKind::Prefix) && t.end == s {
            cands.push((t.start, t.end));
        }
    }
    // sexps and enclosing lists
    let mut off = s;
    if let Some(d) = datum_at_caret(p, off) {
        cands.push((p.start_of(d), p.end_of(d)));
    }
    while let Some(o) = p.enclosing_open(off) {
        let Some(c) = p.partner[o] else { break };
        cands.push((p.tokens[o].end, p.tokens[c].start));
        let list = Datum { first: o, last: c };
        // include reader prefixes directly attached to the bracket
        let mut first = list.first;
        while first > 0 && p.tokens[first - 1].kind == TokenKind::Prefix && p.tokens[first - 1].end == p.tokens[first].start {
            first -= 1;
        }
        cands.push((p.tokens[first].start, p.tokens[c].end));
        off = p.tokens[o].start;
    }
    cands.push((0, src.len()));
    cands
        .into_iter()
        .filter(|&(cs, ce)| cs <= s && e <= ce && (cs, ce) != (s, e) && cs < ce)
        .min_by_key(|&(cs, ce)| ce - cs)
        .unwrap_or((s, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::Dialect;
    use crate::parse::parse;

    fn p(s: &str) -> Parsed {
        parse(s, Dialect::CommonLisp)
    }

    /// `|` marks the caret in the fixture; returns (text, caret).
    fn fx(s: &str) -> (String, usize) {
        let c = s.find('|').unwrap();
        (s.replacen('|', "", 1), c)
    }

    fn show(r: &Rewrite) -> String {
        let mut t = r.text.clone();
        t.insert(r.caret, '|');
        t
    }

    #[test]
    fn slurp_and_barf_forward() {
        let (s, c) = fx("(a |b) c d");
        let r = forward_slurp(&s, &p(&s), c).unwrap();
        assert_eq!(show(&r), "(a |b c) d");
        let (s, c) = fx("(a |b c)");
        let r = forward_barf(&s, &p(&s), c).unwrap();
        assert_eq!(show(&r), "(a |b) c");
        let (s, c) = fx("(|a)");
        assert_eq!(show(&forward_barf(&s, &p(&s), c).unwrap()), "(|) a");
    }

    #[test]
    fn slurp_and_barf_backward() {
        let (s, c) = fx("a (b |c)");
        let r = backward_slurp(&s, &p(&s), c).unwrap();
        assert_eq!(r.text, "(a b c)");
        let (s, c) = fx("(a |b c)");
        let r = backward_barf(&s, &p(&s), c).unwrap();
        assert_eq!(r.text, "a (b c)");
    }

    #[test]
    fn raise_splice_wrap_kill() {
        let (s, c) = fx("(a (b |c) d)");
        assert_eq!(raise(&s, &p(&s), c).unwrap().text, "(a c d)");
        let (s, c) = fx("(a (b |c) d)");
        assert_eq!(splice(&s, &p(&s), c).unwrap().text, "(a b c d)");
        let (s, c) = fx("(a |b c)");
        assert_eq!(wrap_round(&s, &p(&s), c).unwrap().text, "(a (b) c)");
        let (s, c) = fx("(a |b c)");
        assert_eq!(kill_sexp(&s, &p(&s), c).unwrap().text, "(a c)");
    }

    #[test]
    fn navigation() {
        let s = "(a (b c) d)";
        let r = p(s);
        assert_eq!(forward_sexp(&r, 1), Some(2));
        assert_eq!(forward_sexp(&r, 2), Some(8));
        assert_eq!(backward_sexp(&r, 8), Some(3));
        assert_eq!(up_list(&r, 5), Some(3));
        assert_eq!(down_list(&r, 2), Some(4));
    }

    #[test]
    fn toplevel_form() {
        let s = "(a)\n\n(b (c))\n";
        let r = p(s);
        assert_eq!(toplevel_form_at(&r, 6), Some((5, 12)));
        assert_eq!(toplevel_form_at(&r, 4), Some((0, 3)));
        assert_eq!(toplevel_form_at(&r, 1), Some((0, 3)));
    }

    #[test]
    fn sexp_before_caret() {
        let s = "(+ 1 2) (* 3 4) ";
        let r = p(s);
        assert_eq!(sexp_before(s, &r, 15), Some((8, 15)));
        assert_eq!(sexp_before(s, &r, 16), Some((8, 15)));
        assert_eq!(sexp_before(s, &r, 7), Some((0, 7)));
    }

    #[test]
    fn extend_selection_ladder() {
        let s = "(defun f (x) (+ x 1))";
        let r = p(s);
        let mut sel = (15, 15); // inside "+"
        let mut seen = vec![];
        for _ in 0..8 {
            let n = extend_selection(s, &r, sel);
            if n == sel {
                break;
            }
            seen.push(&s[n.0..n.1]);
            sel = n;
        }
        assert_eq!(seen[0], "+");
        assert!(seen.contains(&"+ x 1"));
        assert!(seen.contains(&"(+ x 1)"));
        assert_eq!(*seen.last().unwrap(), s);
        // strictly growing
        let lens: Vec<_> = seen.iter().map(|x| x.len()).collect();
        assert!(lens.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn extend_selection_string_contents() {
        let s = "(f \"hello world\")";
        let r = p(s);
        let a = extend_selection(s, &r, (6, 6));
        assert_eq!(&s[a.0..a.1], "hello world");
        let b = extend_selection(s, &r, a);
        assert_eq!(&s[b.0..b.1], "\"hello world\"");
    }
}
