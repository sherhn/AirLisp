//! Marker comments - `# TODO: ...`, `// FIXME(me) ...`, `; INFO: ...` - which an editor colours so that they stand
//! out of the other comments, as PyCharm does for its TODO patterns.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoKind {
    /// Something to do or to know: TODO, NOTE, INFO.
    Todo,
    /// Something wrong or risky: FIXME, XXX, HACK, BUG, ALERT, WARN, WARNING, DEPRECATED.
    Alert,
}

const TODO_WORDS: &[&str] = &["TODO", "NOTE", "INFO", "OPTIMIZE", "REVIEW"];
const ALERT_WORDS: &[&str] = &["FIXME", "XXX", "HACK", "BUG", "ALERT", "WARN", "WARNING", "DEPRECATED", "ATTENTION", "DANGER"];

fn kind_of(word: &str, followed_by_colon: bool) -> Option<TodoKind> {
    if TODO_WORDS.contains(&word) {
        return Some(TodoKind::Todo);
    }
    if ALERT_WORDS.contains(&word) {
        return Some(TodoKind::Alert);
    }
    // `todo:` and `Fixme:` are meant just as much; plain `todo` in a sentence is not
    if followed_by_colon {
        match word.to_ascii_uppercase().as_str() {
            "TODO" | "NOTE" | "INFO" => return Some(TodoKind::Todo),
            "FIXME" | "BUG" | "HACK" | "ALERT" | "WARNING" => return Some(TodoKind::Alert),
            _ => {}
        }
    }
    None
}

/// Where the marker starts in a piece of comment text (a line, or the part of one the comment covers), and its kind:
/// the marker and everything after it is what gets coloured. `None` when the text carries no marker.
pub fn marker_in(comment: &str) -> Option<(usize, TodoKind)> {
    let mut start: Option<usize> = None;
    for (i, c) in comment.char_indices().chain(std::iter::once((comment.len(), ' '))) {
        let word_char = c.is_alphanumeric() || c == '_';
        match (start, word_char) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                start = None;
                let word = &comment[s..i];
                if let Some(k) = kind_of(word, comment[i..].starts_with(':')) {
                    return Some((s, k));
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_are_found_with_their_kind() {
        assert_eq!(marker_in("# TODO: write it"), Some((2, TodoKind::Todo)));
        assert_eq!(marker_in("// FIXME(vlad) broken"), Some((3, TodoKind::Alert)));
        assert_eq!(marker_in("; INFO: the port is 80"), Some((2, TodoKind::Todo)));
        assert_eq!(marker_in("#ALERT: careful"), Some((1, TodoKind::Alert)));
        assert_eq!(marker_in("/* NOTE this */"), Some((3, TodoKind::Todo)));
        assert_eq!(marker_in("# todo: later"), Some((2, TodoKind::Todo)));
    }

    #[test]
    fn ordinary_comments_are_left_alone() {
        assert_eq!(marker_in("# a todo list and a note on notes"), None);
        assert_eq!(marker_in("// Fixed the bug in TODOs"), None);
        assert_eq!(marker_in("# autoTODO"), None);
        assert_eq!(marker_in(""), None);
    }
}
