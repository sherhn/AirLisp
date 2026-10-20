//! Dispatch of editor level commands (by registry id) onto the [`Editor`].
//! The GUI and the CLI both go through this, so behaviour is identical and testable headless.

use crate::editor::{Editor, SexpMove, StructuralOp};

/// Context some commands need (viewport size).
#[derive(Clone, Copy, Debug)]
pub struct ExecCtx {
    pub page_lines: usize,
}

impl Default for ExecCtx {
    fn default() -> Self {
        ExecCtx { page_lines: 30 }
    }
}

/// Command ids that [`apply`] understands.
pub const EDITOR_COMMANDS: &[&str] = &[
    "edit.undo", "edit.redo", "edit.select_all", "edit.extend_selection", "edit.shrink_selection", "edit.duplicate",
    "edit.delete_line", "edit.delete_to_line_end", "edit.delete_word_back", "edit.delete_word_forward", "edit.move_line_up",
    "edit.move_line_down", "edit.join_lines", "edit.new_line_below", "edit.new_line_above", "edit.toggle_case", "edit.indent",
    "edit.unindent", "edit.newline", "edit.backspace", "edit.delete", "caret.left", "caret.right", "caret.up", "caret.down",
    "caret.word_left", "caret.word_right", "caret.line_start", "caret.line_end", "caret.doc_start", "caret.doc_end",
    "caret.page_up", "caret.page_down", "select.left", "select.right", "select.up", "select.down", "select.word_left",
    "select.word_right", "select.line_start", "select.line_end", "select.doc_start", "select.doc_end", "select.page_up",
    "select.page_down", "code.reformat", "code.auto_indent", "code.comment_line", "nav.matching_bracket", "lisp.slurp_forward",
    "lisp.barf_forward", "lisp.slurp_backward", "lisp.barf_backward", "lisp.raise", "lisp.splice", "lisp.wrap", "lisp.kill_sexp",
    "lisp.forward_sexp", "lisp.backward_sexp", "lisp.up_list", "lisp.down_list",
];

pub fn apply(ed: &mut Editor, id: &str) -> bool {
    apply_with(ed, id, ExecCtx::default())
}

/// Run an editor command. Returns `false` if `id` is not an editor command.
pub fn apply_with(ed: &mut Editor, id: &str, ctx: ExecCtx) -> bool {
    let page = ctx.page_lines.max(1) as isize;
    match id {
        "edit.undo" => {
            ed.undo();
        }
        "edit.redo" => {
            ed.redo();
        }
        "edit.select_all" => ed.select_all(),
        "edit.extend_selection" => ed.extend_selection(),
        "edit.shrink_selection" => ed.shrink_selection(),
        "edit.duplicate" => ed.duplicate(),
        "edit.delete_line" => ed.delete_line(),
        "edit.delete_to_line_end" => ed.delete_to_line_end(),
        "edit.delete_word_back" => ed.delete_word_back(),
        "edit.delete_word_forward" => ed.delete_word_forward(),
        "edit.move_line_up" => ed.move_lines(true),
        "edit.move_line_down" => ed.move_lines(false),
        "edit.join_lines" => ed.join_lines(),
        "edit.new_line_below" => ed.new_line_below(),
        "edit.new_line_above" => ed.new_line_above(),
        "edit.toggle_case" => ed.toggle_case(),
        "edit.indent" => ed.indent_lines(),
        "edit.unindent" => ed.unindent_lines(),
        "edit.newline" => ed.insert_newline(),
        "edit.backspace" => ed.backspace(),
        "edit.delete" => ed.delete_forward(),
        "caret.left" => ed.move_left(false),
        "caret.right" => ed.move_right(false),
        "caret.up" => ed.move_vertical(-1, false),
        "caret.down" => ed.move_vertical(1, false),
        "caret.word_left" => ed.move_word_left(false),
        "caret.word_right" => ed.move_word_right(false),
        "caret.line_start" => ed.move_line_start(false),
        "caret.line_end" => ed.move_line_end(false),
        "caret.doc_start" => ed.move_doc_start(false),
        "caret.doc_end" => ed.move_doc_end(false),
        "caret.page_up" => ed.move_vertical(-page, false),
        "caret.page_down" => ed.move_vertical(page, false),
        "select.left" => ed.move_left(true),
        "select.right" => ed.move_right(true),
        "select.up" => ed.move_vertical(-1, true),
        "select.down" => ed.move_vertical(1, true),
        "select.word_left" => ed.move_word_left(true),
        "select.word_right" => ed.move_word_right(true),
        "select.line_start" => ed.move_line_start(true),
        "select.line_end" => ed.move_line_end(true),
        "select.doc_start" => ed.move_doc_start(true),
        "select.doc_end" => ed.move_doc_end(true),
        "select.page_up" => ed.move_vertical(-page, true),
        "select.page_down" => ed.move_vertical(page, true),
        "code.reformat" => ed.reformat(),
        "code.auto_indent" => ed.auto_indent_lines(),
        "code.comment_line" => ed.toggle_line_comment(),
        "nav.matching_bracket" => {
            ed.goto_matching_bracket(false);
        }
        "lisp.slurp_forward" => {
            ed.structural(StructuralOp::SlurpForward);
        }
        "lisp.barf_forward" => {
            ed.structural(StructuralOp::BarfForward);
        }
        "lisp.slurp_backward" => {
            ed.structural(StructuralOp::SlurpBackward);
        }
        "lisp.barf_backward" => {
            ed.structural(StructuralOp::BarfBackward);
        }
        "lisp.raise" => {
            ed.structural(StructuralOp::Raise);
        }
        "lisp.splice" => {
            ed.structural(StructuralOp::Splice);
        }
        "lisp.wrap" => {
            ed.structural(StructuralOp::WrapRound);
        }
        "lisp.kill_sexp" => {
            ed.structural(StructuralOp::KillSexp);
        }
        "lisp.forward_sexp" => {
            ed.sexp_move(SexpMove::Forward, false);
        }
        "lisp.backward_sexp" => {
            ed.sexp_move(SexpMove::Backward, false);
        }
        "lisp.up_list" => {
            ed.sexp_move(SexpMove::Up, false);
        }
        "lisp.down_list" => {
            ed.sexp_move(SexpMove::Down, false);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands;
    use lisp_syntax::Dialect;

    #[test]
    fn every_listed_command_is_registered_and_handled() {
        for id in EDITOR_COMMANDS {
            assert!(commands::by_id(id).is_some(), "{id} not in registry");
            let mut ed = Editor::new("(a b)\n(c d)\n", Dialect::CommonLisp);
            ed.set_caret(2, false);
            assert!(apply(&mut ed, id), "{id} not handled");
        }
    }

    #[test]
    fn unknown_and_app_commands_are_not_handled() {
        let mut ed = Editor::new("", Dialect::CommonLisp);
        assert!(!apply(&mut ed, "file.save"));
        assert!(!apply(&mut ed, "nope"));
    }

    #[test]
    fn registry_editor_commands_are_all_dispatchable() {
        // every caret/select/lisp/edit.* command that mutates or moves the caret must be known here
        for c in commands::COMMANDS {
            let editorish = c.id.starts_with("caret.") || c.id.starts_with("select.") || c.id.starts_with("lisp.");
            if editorish {
                assert!(EDITOR_COMMANDS.contains(&c.id), "{} missing from EDITOR_COMMANDS", c.id);
            }
        }
    }

    #[test]
    fn page_movement_uses_context() {
        let text: String = (0..100).map(|i| format!("line{i}\n")).collect();
        let mut ed = Editor::new(&text, Dialect::CommonLisp);
        apply_with(&mut ed, "caret.page_down", ExecCtx { page_lines: 10 });
        assert_eq!(ed.line_col(ed.caret()).0, 10);
        apply_with(&mut ed, "select.page_down", ExecCtx { page_lines: 5 });
        assert_eq!(ed.line_col(ed.caret()).0, 15);
        assert!(ed.has_selection());
    }

    #[test]
    fn scripted_session_matches_expectation() {
        let mut ed = Editor::new("(defun f (x)\n(+ x 1))\n", Dialect::CommonLisp);
        for id in ["code.reformat", "caret.doc_end", "edit.undo", "edit.redo"] {
            assert!(apply(&mut ed, id));
        }
        assert_eq!(ed.text(), "(defun f (x)\n  (+ x 1))\n");
    }
}
