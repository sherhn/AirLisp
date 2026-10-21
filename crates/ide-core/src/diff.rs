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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Same,
    /// A line that exists on both sides but differs.
    Changed,
    Removed,
    Added,
}

/// One side of a [`SideRow`]: a line of the old or the new text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideCell {
    /// 1-based line number.
    pub no: usize,
    /// Display text, tabs expanded to four spaces.
    pub text: String,
    /// For [`RowKind::Changed`]: the character range that differs from the other side.
    pub hl: Option<(usize, usize)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideRow {
    pub kind: RowKind,
    pub left: Option<SideCell>,
    pub right: Option<SideCell>,
}

/// The old and the new text laid out in two aligned columns (what a side-by-side diff viewer shows).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SideBySide {
    pub rows: Vec<SideRow>,
    /// `[start, end)` row ranges of the change blocks, in order.
    pub hunks: Vec<(usize, usize)>,
    pub added: usize,
    pub removed: usize,
}

/// Longest common character prefix and suffix of two lines, as the differing middle ranges.
fn changed_span(a: &str, b: &str) -> (Option<(usize, usize)>, Option<(usize, usize)>) {
    let (ac, bc): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut pre = 0;
    while pre < ac.len() && pre < bc.len() && ac[pre] == bc[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < ac.len() - pre && suf < bc.len() - pre && ac[ac.len() - 1 - suf] == bc[bc.len() - 1 - suf] {
        suf += 1;
    }
    if pre == 0 && suf == 0 {
        return (None, None);
    }
    (Some((pre, ac.len() - suf)).filter(|r| r.0 < r.1), Some((pre, bc.len() - suf)).filter(|r| r.0 < r.1))
}

fn expand(s: &str) -> String {
    s.replace('\t', "    ")
}

/// Align `new` against `old` for a side-by-side view: unchanged lines share a row, a block of removed and added
/// lines is paired line by line (the longer side continues alone).
pub fn side_by_side(old: &str, new: &str) -> SideBySide {
    let lines = diff_lines(old, new);
    let mut out = SideBySide::default();
    let mut i = 0;
    while i < lines.len() {
        let l = &lines[i];
        if l.kind == LineKind::Same {
            let cell = |no: Option<usize>| no.map(|no| SideCell { no, text: expand(&l.text), hl: None });
            out.rows.push(SideRow { kind: RowKind::Same, left: cell(l.old_no), right: cell(l.new_no) });
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && lines[i].kind != LineKind::Same {
            i += 1;
        }
        let block = &lines[start..i];
        let removed: Vec<&DiffLine> = block.iter().filter(|l| l.kind == LineKind::Removed).collect();
        let added: Vec<&DiffLine> = block.iter().filter(|l| l.kind == LineKind::Added).collect();
        out.removed += removed.len();
        out.added += added.len();
        let first_row = out.rows.len();
        for k in 0..removed.len().max(added.len()) {
            let cell = |l: &DiffLine, no: Option<usize>, hl| no.map(|no| SideCell { no, text: expand(&l.text), hl });
            let row = match (removed.get(k), added.get(k)) {
                (Some(r), Some(a)) => {
                    let (hl_l, hl_r) = changed_span(&expand(&r.text), &expand(&a.text));
                    SideRow { kind: RowKind::Changed, left: cell(r, r.old_no, hl_l), right: cell(a, a.new_no, hl_r) }
                }
                (Some(r), None) => SideRow { kind: RowKind::Removed, left: cell(r, r.old_no, None), right: None },
                (None, Some(a)) => SideRow { kind: RowKind::Added, left: None, right: cell(a, a.new_no, None) },
                (None, None) => unreachable!("k is below the longer side"),
            };
            out.rows.push(row);
        }
        out.hunks.push((first_row, out.rows.len()));
    }
    out
}

/// A contiguous block of changed lines: what the gutter marker next to it stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    /// 0-based first line in the new text. For a pure deletion `new_len` is 0 and this is the line the removed
    /// text sat in front of (it may equal the line count when the deletion was at the very end).
    pub new_start: usize,
    pub new_len: usize,
    /// 0-based first line in the old text.
    pub old_start: usize,
    /// The old lines of the block (empty for a pure insertion).
    pub old_lines: Vec<String>,
}

/// All changed blocks of `new` relative to `old`, in file order.
pub fn hunks(old: &str, new: &str) -> Vec<Hunk> {
    let lines = diff_lines(old, new);
    let new_count = new.lines().count();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].kind == LineKind::Same {
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && lines[i].kind != LineKind::Same {
            i += 1;
        }
        let block = &lines[start..i];
        let old_lines: Vec<String> = block.iter().filter(|l| l.kind == LineKind::Removed).map(|l| l.text.clone()).collect();
        let added: Vec<usize> = block.iter().filter_map(|l| l.new_no).collect();
        let old_start = block.iter().find_map(|l| l.old_no).map_or(0, |n| n - 1);
        let (new_start, new_len) = match added.first() {
            Some(&first) => (first - 1, added.len()),
            None => (lines[i..].iter().find_map(|l| l.new_no).map_or(new_count, |n| n - 1), 0),
        };
        out.push(Hunk { new_start, new_len, old_start, old_lines });
    }
    out
}

/// The hunk a gutter marker on `line` (0-based, as in [`Mark::line`]) belongs to.
pub fn hunk_at(hunks: &[Hunk], line: usize, new_line_count: usize) -> Option<&Hunk> {
    hunks.iter().find(|h| if h.new_len > 0 { line >= h.new_start && line < h.new_start + h.new_len } else { h.new_start.min(new_line_count.saturating_sub(1)) == line })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunks_carry_the_old_lines() {
        let old = "a\nb\nc\nd\ne";
        let new = "a\nB\nc\nx\ny\ne";
        let h = hunks(old, new);
        assert_eq!(h.len(), 2);
        assert_eq!((h[0].new_start, h[0].new_len, h[0].old_start), (1, 1, 1));
        assert_eq!(h[0].old_lines, vec!["b"]);
        assert_eq!((h[1].new_start, h[1].new_len), (3, 2));
        assert_eq!(h[1].old_lines, vec!["d"]);
        // pure insertion has no old code, pure deletion has no new lines
        assert!(hunks("a\nb", "a\nNEW\nb")[0].old_lines.is_empty());
        let del = hunks("a\nb\nc", "a\nc");
        assert_eq!((del[0].new_start, del[0].new_len, del[0].old_lines.clone()), (1, 0, vec!["b".to_string()]));
        assert!(hunks("same", "same").is_empty());
    }

    #[test]
    fn hunk_lookup_matches_gutter_marks() {
        let old = "a\nb\nc\nd\ne";
        let new = "a\nB\nc\nx\ny\ne";
        let h = hunks(old, new);
        for m in gutter_marks(old, new) {
            assert!(hunk_at(&h, m.line, new.lines().count()).is_some(), "mark {m:?}");
        }
        assert!(hunk_at(&h, 0, 6).is_none());
        // deletion at the end: the mark sits on the last line, the insertion point is past it
        let h = hunks("a\nb\nc", "a\nb");
        let marks = gutter_marks("a\nb\nc", "a\nb");
        assert_eq!(h[0].new_start, 2);
        assert_eq!(hunk_at(&h, marks[0].line, 2).map(|x| x.old_lines.clone()), Some(vec!["c".to_string()]));
    }

    #[test]
    fn side_by_side_pairs_changed_lines_and_keeps_same_rows_aligned() {
        let sbs = side_by_side("a
b
c
d", "a
B
c
d
e");
        let kinds: Vec<RowKind> = sbs.rows.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, vec![RowKind::Same, RowKind::Changed, RowKind::Same, RowKind::Same, RowKind::Added]);
        assert_eq!(sbs.hunks, vec![(1, 2), (4, 5)]);
        assert_eq!((sbs.added, sbs.removed), (2, 1));
        let r = &sbs.rows[1];
        assert_eq!((r.left.as_ref().unwrap().no, r.right.as_ref().unwrap().no), (2, 2));
        assert_eq!(r.left.as_ref().unwrap().text, "b");
        // the added line has nothing on the left, and its number on the right
        assert!(sbs.rows[4].left.is_none());
        assert_eq!(sbs.rows[4].right.as_ref().unwrap().no, 5);
    }

    #[test]
    fn side_by_side_longer_side_continues_alone() {
        let sbs = side_by_side("x
1
2
3
y", "x
A
y");
        let kinds: Vec<RowKind> = sbs.rows.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, vec![RowKind::Same, RowKind::Changed, RowKind::Removed, RowKind::Removed, RowKind::Same]);
        assert_eq!(sbs.hunks, vec![(1, 4)]);
        assert_eq!((sbs.added, sbs.removed), (1, 3));
        assert!(sbs.rows[2].right.is_none());
        assert_eq!(sbs.rows[3].left.as_ref().unwrap().text, "3");
    }

    #[test]
    fn side_by_side_marks_the_differing_characters_and_expands_tabs() {
        let sbs = side_by_side("(* x 3)", "(* x 42)");
        let r = &sbs.rows[0];
        assert_eq!(r.left.as_ref().unwrap().hl, Some((5, 6)));
        assert_eq!(r.right.as_ref().unwrap().hl, Some((5, 7)));
        let sbs = side_by_side("	one", "	two");
        assert_eq!(sbs.rows[0].left.as_ref().unwrap().text, "    one");
        assert_eq!(sbs.rows[0].left.as_ref().unwrap().hl, Some((4, 7)));
        // completely different lines are not given a (meaningless) inner highlight
        assert_eq!(side_by_side("abc", "xyz").rows[0].left.as_ref().unwrap().hl, None);
    }

    #[test]
    fn side_by_side_of_equal_and_empty_texts() {
        assert!(side_by_side("a
b", "a
b").hunks.is_empty());
        let all_new = side_by_side("", "a
b");
        assert_eq!((all_new.added, all_new.removed, all_new.hunks.clone()), (2, 0, vec![(0, 2)]));
        let gone = side_by_side("a
b", "");
        assert_eq!((gone.added, gone.removed), (0, 2));
        assert!(gone.rows.iter().all(|r| r.right.is_none()));
    }

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
