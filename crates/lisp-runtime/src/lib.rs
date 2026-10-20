//! Talking to a real Lisp: discovery, an interactive REPL session with a structured protocol,
//! running files and parsing compiler diagnostics.

pub mod compiler;
pub mod discover;
pub mod protocol;
pub mod repl;
pub mod runner;

pub use discover::Implementation;
pub use repl::{EvalOutcome, ReplEvent, ReplSession};
pub use runner::{RunEvent, RunHandle};

use std::process::Command;

/// Hide the console window of spawned processes on Windows.
pub(crate) fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}
