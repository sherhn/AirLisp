//! Line diff used for the change markers in the gutter and for the diff viewer.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Same,
    Added,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    /// 1-based line numbers in the old / new text (None on the side where the line does not exist).
    pub old_no: Option<usize>,
    pub new_no: Option<usize>,
    pub text: String,
}

/// Longest-common-subsequence diff of two line lists, with common prefix/suffix trimming.
/// Falls back to "replace everything in the middle" when the middle is huge.
pub fn diff_lines(old: &str, new: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf] {
        suf += 1;
    }
    let am = &a[pre..a.len() - suf];
    let bm = &b[pre..b.len() - suf];
    let mut out = Vec::with_capacity(a.len().max(b.len()));
    for i in 0..pre {
        out.push(DiffLine { kind: LineKind::Same, old_no: Some(i + 1), new_no: Some(i + 1), text: a[i].to_string() });
    }
    let (n, m) = (am.len(), bm.len());
    if n * m > 6_000_000 {
        for (i, l) in am.iter().enumerate() {
            out.push(DiffLine { kind: LineKind::Removed, old_no: Some(pre + i + 1), new_no: None, text: l.to_string() });
        }
        for (j, l) in bm.iter().enumerate() {
            out.push(DiffLine { kind: LineKind::Added, old_no: None, new_no: Some(pre + j + 1), text: l.to_string() });
        }
    } else {
        // lcs[i][j] = LCS length of am[i..], bm[j..]
        let mut lcs = vec![0u32; (n + 1) * (m + 1)];
        let w = m + 1;
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                lcs[i * w + j] = if am[i] == bm[j] { lcs[(i + 1) * w + j + 1] + 1 } else { lcs[(i + 1) * w + j].max(lcs[i * w + j + 1]) };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n || j < m {
            if i < n && j < m && am[i] == bm[j] {
                out.push(DiffLine { kind: LineKind::Same, old_no: Some(pre + i + 1), new_no: Some(pre + j + 1), text: am[i].to_string() });
                i += 1;
                j += 1;
            } else if i < n && (j >= m || lcs[(i + 1) * w + j] >= lcs[i * w + j + 1]) {
                out.push(DiffLine { kind: LineKind::Removed, old_no: Some(pre + i + 1), new_no: None, text: am[i].to_string() });
                i += 1;
            } else {
                out.push(DiffLine { kind: LineKind::Added, old_no: None, new_no: Some(pre + j + 1), text: bm[j].to_string() });
                j += 1;
            }
        }
    }
    for k in 0..suf {
        let (oi, ni) = (a.len() - suf + k, b.len() - suf + k);
        out.push(DiffLine { kind: LineKind::Same, old_no: Some(oi + 1), new_no: Some(ni + 1), text: a[oi].to_string() });
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    Added,
    Modified,
    /// Lines were removed just before `line`.
    Deleted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark {
    /// 0-based line in the new text.
    pub line: usize,
    pub kind: MarkKind,
}

/// Gutter change markers for `new` relative to `old` (what the IDE shows next to line numbers).
pub fn gutter_marks(old: &str, new: &str) -> Vec<Mark> {
    let lines = diff_lines(old, new);
    let mut marks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].kind == LineKind::Same {
            i += 1;
            continue;
        }
        // a change block: consecutive non-Same lines
        let start = i;
        while i < lines.len() && lines[i].kind != LineKind::Same {
            i += 1;
        }
        let block = &lines[start..i];
        let removed = block.iter().filter(|l| l.kind == LineKind::Removed).count();
        let added: Vec<usize> = block.iter().filter_map(|l| l.new_no).collect();
        if added.is_empty() {
            // pure deletion: mark the line after the gap (or the last line)
            let at = lines[i..].iter().find_map(|l| l.new_no).map_or_else(|| new.lines().count().saturating_sub(1), |n| n - 1);
            marks.push(Mark { line: at, kind: MarkKind::Deleted });
        } else {
            let kind = if removed > 0 { MarkKind::Modified } else { MarkKind::Added };
            for n in added {
                marks.push(Mark { line: n - 1, kind });
            }
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(old: &str, new: &str) -> String {
        diff_lines(old, new)
            .iter()
            .map(|l| match l.kind {
                LineKind::Same => '=',
                LineKind::Added => '+',
                LineKind::Removed => '-',
            })
            .collect()
    }

    #[test]
    fn identical_and_empty() {
        assert_eq!(kinds("a\nb", "a\nb"), "==");
        assert_eq!(kinds("", ""), "");
        assert_eq!(kinds("", "a"), "+");
        assert_eq!(kinds("a", ""), "-");
    }

    #[test]
    fn insert_delete_replace() {
        assert_eq!(kinds("a\nc", "a\nb\nc"), "=+=");
        assert_eq!(kinds("a\nb\nc", "a\nc"), "=-=");
        let k = kinds("a\nb\nc", "a\nX\nc");
        assert!(k == "=-+=" || k == "=+-=", "{k}");
    }

    #[test]
    fn line_numbers_are_consistent() {
        let d = diff_lines("one\ntwo\nthree", "one\n2\nthree\nfour");
        let last = d.last().unwrap();
        assert_eq!((last.kind, last.new_no, last.old_no), (LineKind::Added, Some(4), None));
        let three = d.iter().find(|l| l.text == "three").unwrap();
        assert_eq!((three.old_no, three.new_no), (Some(3), Some(3)));
    }

    #[test]
    fn gutter_marks_added_modified_deleted() {
        let old = "a\nb\nc\nd\ne";
        let new = "a\nB\nc\nx\ny\ne"; // b modified, d replaced by two lines
        let m = gutter_marks(old, new);
        assert_eq!(m.iter().filter(|x| x.kind == MarkKind::Modified).count(), 3, "{m:?}");
        let m = gutter_marks("a\nb\nc", "a\nb\nNEW\nc");
        assert_eq!(m, vec![Mark { line: 2, kind: MarkKind::Added }]);
        let m = gutter_marks("a\nb\nc", "a\nc");
        assert_eq!(m, vec![Mark { line: 1, kind: MarkKind::Deleted }]);
        assert!(gutter_marks("same", "same").is_empty());
    }

    #[test]
    fn deletion_at_end() {
        let m = gutter_marks("a\nb\nc", "a\nb");
        assert_eq!(m, vec![Mark { line: 1, kind: MarkKind::Deleted }]);
    }

    #[test]
    fn large_inputs_do_not_blow_up() {
        let a: String = (0..4000).map(|i| format!("line {i}\n")).collect();
        let b: String = (0..4000).map(|i| if i % 7 == 0 { format!("changed {i}\n") } else { format!("line {i}\n") }).collect();
        let d = diff_lines(&a, &b);
        assert!(d.iter().filter(|l| l.kind == LineKind::Added).count() > 500);
    }
}
